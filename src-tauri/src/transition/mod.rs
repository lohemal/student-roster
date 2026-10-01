//! 학년도 전환 — 다음 학년도 학적을 **새로 만든다.**
//!
//! ```text
//! 원본 학년도 재학생 ──┐
//!                      ├─▶ 계획(plan) ─▶ 사람이 확인 ─▶ 백업 ─▶ 한 트랜잭션
//! 진급 배정 파일 ──────┘
//! ```
//!
//! 지키는 것
//!   * **지난 학년도를 고치지 않는다.** 2026학년도 3-가람-7 은 그대로 두고 2027학년도
//!     4-나리-12 를 만든다. 학생 번호가 같으므로 형제 관계와 이력이 그대로 이어진다.
//!   * **프로그램이 반을 정하지 않는다.** 새 반은 학교가 정해 파일로 넣는다.
//!   * **막는 것과 알리는 것을 나눈다.** 새 반이 없으면 학적을 만들 수 없으니 막고,
//!     주소 미분류처럼 나중에 고칠 수 있는 것은 막지 않는다.
//!   * 미리보기와 결과가 어긋나지 않도록 **트랜잭션 안에서 계획을 다시 세운다.**
//!   * 로그에 학생 이름·생년월일을 남기지 않는다. 학년도와 인원만 남긴다.

pub mod assign;

use std::collections::HashMap;
use std::sync::Mutex;

use chrono::NaiveDate;
use rusqlite::Connection;
use serde::Serialize;

use crate::domain::transition::{self as rules, NumberState, Outcome, ProblemKind};
use crate::domain::{birth, label};
use crate::error::{AppError, AppResult};
use crate::repo::student;
use crate::repo::transition as repo;

use assign::AssignRow;

// ---------------------------------------------------------------
// 읽어 둔 배정 자료
// ---------------------------------------------------------------

/// 읽어 둔 배정 파일. 적용할 때까지 들고 있는다.
///
/// 화면이 되돌려 보낸 값을 그대로 믿지 않기 위해서다 — 가져오기(`import::SessionStore`)와
/// 같은 방법이다.
pub struct AssignSet {
    pub id: String,
    pub file_name: String,
    pub sheet_name: String,
    pub from_year: i32,
    pub rows: Vec<AssignRow>,
}

#[derive(Default)]
pub struct AssignStore(pub Mutex<Option<AssignSet>>);

impl AssignStore {
    pub fn put(&self, s: AssignSet) {
        let mut g = self.0.lock().unwrap_or_else(|e| e.into_inner());
        *g = Some(s);
    }

    pub fn clear(&self) {
        let mut g = self.0.lock().unwrap_or_else(|e| e.into_inner());
        *g = None;
    }

    /// 들고 있는 자료를 복사해 준다. 없으면 None.
    pub fn snapshot(&self) -> Option<AssignSet> {
        let g = self.0.lock().unwrap_or_else(|e| e.into_inner());
        g.as_ref().map(|s| AssignSet {
            id: s.id.clone(),
            file_name: s.file_name.clone(),
            sheet_name: s.sheet_name.clone(),
            from_year: s.from_year,
            rows: s.rows.clone(),
        })
    }
}

// ---------------------------------------------------------------
// 계획
// ---------------------------------------------------------------

/// 확인이 필요한 학생 하나.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Person {
    pub student_id: Option<i64>,
    /// `3-가람-7 홍길동`
    pub label: String,
    /// 무엇이 문제인지 한 줄 더 (엑셀 줄 번호 등)
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    pub kind: ProblemKind,
    pub title: String,
    pub message: String,
    /// 이것이 있으면 전환을 시작하지 않는다
    pub blocking: bool,
    pub students: Vec<Person>,
}

/// 학생별 변경 — 화면 표에 그대로 나간다.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanRow {
    pub student_id: i64,
    pub name: String,
    /// `3-가람-7`
    pub current: String,
    /// `4-다솜-12` · `졸업` · `없음`
    pub next: String,
    /// PROMOTE · GRADUATE · NONE
    pub fate: &'static str,
}

/// `1→2: 187명`
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GradeMove {
    pub from_grade: i32,
    pub to_grade: Option<i32>,
    pub label: String,
    pub count: usize,
}

/// 다음 학년도 예상 학급 현황. 아직 DB 에 없는 값이므로 계획에서 센다.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassPreview {
    pub grade: i32,
    pub class_name: Option<String>,
    pub class_label: String,
    pub male: i64,
    pub female: i64,
    pub unknown: i64,
    pub total: i64,
}

/// 실제로 만들 학적 하나.
#[derive(Debug, Clone)]
pub struct Move {
    pub student_id: i64,
    pub grade: i32,
    pub class_name: String,
    pub class_no: Option<i32>,
}

/// 졸업시킬 학생 하나 (졸업 당시 자리와 함께).
#[derive(Debug, Clone)]
pub struct Leave {
    pub student_id: i64,
    pub grade: i32,
    pub class_name: Option<String>,
    pub class_no: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub from_year: i32,
    pub to_year: i32,
    /// 전환 대상 (원본 학년도 재학생)
    pub target_count: usize,
    pub promote_count: usize,
    pub graduate_count: usize,
    /// 아무것도 만들지 않는 학생 (졸업에서 뺐는데 배정도 없는 경우)
    pub skip_count: usize,
    /// 대상 학년도에 이미 있는 학적 수
    pub existing_count: i64,
    pub rows: Vec<PlanRow>,
    pub by_grade: Vec<GradeMove>,
    pub classes: Vec<ClassPreview>,
    pub problems: Vec<Problem>,
    /// 원본 학년도의 지금 상태. 적용할 때 다시 견준다.
    pub state_key: String,
    pub blocked: bool,
    /// 읽어 둔 배정 파일 이름
    pub assign_file: Option<String>,
    pub assign_rows: usize,

    #[serde(skip)]
    pub moves: Vec<Move>,
    #[serde(skip)]
    pub leaves: Vec<Leave>,
}

struct Bag {
    kind: ProblemKind,
    message: String,
    students: Vec<Person>,
}

impl Bag {
    fn new(kind: ProblemKind, message: impl Into<String>) -> Self {
        Bag {
            kind,
            message: message.into(),
            students: Vec::new(),
        }
    }
}

fn person(seat: &repo::SeatRow, detail: Option<String>) -> Person {
    Person {
        student_id: Some(seat.student_id),
        label: format!("{} {}", seat.seat_label(), seat.name),
        detail,
    }
}

/// 배정 줄과 학생을 짝지은 결과.
struct Matched {
    /// seats 안 자리 번호 → 배정 줄
    by_seat: HashMap<usize, AssignRow>,
}

/// 파일의 줄을 지금 학생과 짝짓는다.
///
/// 차례는 **학생번호 → 기존 학년·반·번호 → 이름+생년월일** 이다. 파일은 사람이
/// 고칠 수 있으므로 학생번호도 DB 와 견주어 확인한다 — 번호만 믿지 않는다.
fn match_rows(
    seats: &[repo::SeatRow],
    from_year: i32,
    rows: &[AssignRow],
    today: NaiveDate,
    bags: &mut Vec<Bag>,
) -> Matched {
    let mut by_id: HashMap<i64, usize> = HashMap::new();
    let mut by_place: HashMap<(i32, String, i32), Vec<usize>> = HashMap::new();
    let mut by_name_birth: HashMap<(String, String), Vec<usize>> = HashMap::new();
    for (i, s) in seats.iter().enumerate() {
        by_id.insert(s.student_id, i);
        if let (Some(cn), Some(no)) = (s.class_name.as_deref(), s.class_no) {
            by_place
                .entry((s.grade, cn.trim().to_string(), no))
                .or_default()
                .push(i);
        }
        if let Some(b) = s.birth_date.as_deref() {
            by_name_birth
                .entry((s.name.trim().to_string(), b.to_string()))
                .or_default()
                .push(i);
        }
    }

    let mut unknown = Bag::new(
        ProblemKind::UnknownStudent,
        "배정 자료의 학생을 명단에서 찾지 못했습니다. 이 학년도 재학생이 맞는지 확인해 주세요.",
    );
    let mut dup = Bag::new(
        ProblemKind::Duplicated,
        "같은 학생이 배정 자료에 두 번 넘게 나옵니다. 한 줄만 남겨 주세요.",
    );
    let mut name_bad = Bag::new(
        ProblemKind::NameMismatch,
        "배정 자료의 이름이 저장된 이름과 다릅니다. 다른 학생일 수 있습니다.",
    );
    let mut wrong_year = Bag::new(
        ProblemKind::WrongYear,
        format!("배정 자료의 학년도가 {from_year}학년도가 아닙니다."),
    );
    let mut stale = Bag::new(
        ProblemKind::StaleSeat,
        "배정 자료에 적힌 기존 학년·반·번호가 지금 학적과 다릅니다. 파일을 만든 뒤에 바뀐 것 같습니다.",
    );

    let mut out: HashMap<usize, AssignRow> = HashMap::new();
    for row in rows {
        let at = format!("{}줄", row.excel_row);

        if let Some(y) = row.school_year {
            if y != from_year {
                wrong_year.students.push(Person {
                    student_id: row.student_id,
                    label: format!("{at} {}", row.name),
                    detail: Some(format!("파일에는 {y}학년도")),
                });
                continue;
            }
        }

        // 1) 학생번호
        let found = row
            .student_id
            .and_then(|id| by_id.get(&id).copied())
            // 2) 기존 학년·반·번호
            .or_else(|| {
                let (g, cn, no) = (row.old_grade?, row.old_class.clone()?, row.old_no?);
                match by_place.get(&(g, cn.trim().to_string(), no)) {
                    Some(v) if v.len() == 1 => Some(v[0]),
                    _ => None,
                }
            })
            // 3) 이름 + 생년월일
            .or_else(|| {
                let iso = birth::parse(row.birth.as_deref()?, today).date?;
                match by_name_birth.get(&(row.name.trim().to_string(), iso.to_string())) {
                    Some(v) if v.len() == 1 => Some(v[0]),
                    _ => None,
                }
            });

        let Some(idx) = found else {
            unknown.students.push(Person {
                student_id: row.student_id,
                label: format!("{at} {}", row.name),
                detail: row
                    .old_grade
                    .map(|g| format!("파일: {g}학년 {}", row.old_class.clone().unwrap_or_default())),
            });
            continue;
        };
        let seat = &seats[idx];

        // 학생번호로 찾았더라도 이름이 다르면 다른 학생일 수 있다
        if !row.name.trim().is_empty() && row.name.trim() != seat.name.trim() {
            name_bad
                .students
                .push(person(seat, Some(format!("{at}: 파일 '{}'", row.name.trim()))));
            continue;
        }

        if out.contains_key(&idx) {
            dup.students.push(person(seat, Some(at)));
            continue;
        }

        // 기존 자리가 다르면 알리기만 한다 — 배정 자체는 살린다
        let same_place = row.old_grade.map(|g| g == seat.grade).unwrap_or(true)
            && row
                .old_class
                .as_deref()
                .map(|c| Some(c.trim()) == seat.class_name.as_deref().map(str::trim))
                .unwrap_or(true)
            && row.old_no.map(|n| Some(n) == seat.class_no).unwrap_or(true);
        if !same_place {
            stale
                .students
                .push(person(seat, Some(format!("지금은 {}", seat.seat_label()))));
        }

        out.insert(idx, row.clone());
    }

    for bag in [unknown, dup, name_bad, wrong_year, stale] {
        if !bag.students.is_empty() {
            bags.push(bag);
        }
    }
    Matched { by_seat: out }
}

/// 전환 계획을 세운다. **DB 를 고치지 않는다.**
pub fn build(
    c: &Connection,
    from_year: i32,
    to_year: i32,
    assigns: Option<&AssignSet>,
    exclude_graduation: &[i64],
    today: NaiveDate,
) -> AppResult<Plan> {
    if to_year <= from_year {
        return Err(AppError::invalid(
            "대상 학년도는 원본 학년도보다 뒤여야 합니다.",
        ));
    }

    let seats = repo::seats(c, from_year, today)?;
    let state_key = repo::state_key(c, from_year, today)?;
    let target = repo::year_state(c, to_year, today)?;
    let mut bags: Vec<Bag> = Vec::new();

    // 대상 학년도가 비어 있어야 한다 — 조용히 섞지 않는다
    if target.enrollments > 0 {
        bags.push(Bag::new(
            ProblemKind::TargetNotEmpty,
            format!(
                "{to_year}학년도에 이미 학적 {}건이 있습니다. 먼저 확인한 뒤 진행해 주세요.",
                target.enrollments
            ),
        ));
    }
    if let Some(at) = repo::done_before(c, from_year, to_year)? {
        bags.push(Bag::new(
            ProblemKind::AlreadyDone,
            format!("{from_year}→{to_year} 전환을 {at} 에 이미 실행했습니다."),
        ));
    }

    let matched = match assigns {
        Some(set) => match_rows(&seats, from_year, &set.rows, today, &mut bags),
        None => Matched {
            by_seat: HashMap::new(),
        },
    };

    // ---- 학생별로 무엇을 할지 정한다 ----
    let excluded: Vec<i64> = exclude_graduation.to_vec();
    let mut no_assign = Bag::new(
        ProblemKind::NoAssign,
        "배정 자료에 없는 학생입니다. 새 반을 알 수 없어 다음 학년도 학적을 만들 수 없습니다.",
    );
    let mut no_class = Bag::new(
        ProblemKind::NoClass,
        "새 반이 비어 있습니다. 반을 적어 주세요 — 프로그램이 예전 반을 그대로 쓰지 않습니다.",
    );
    let mut bad_grade = Bag::new(
        ProblemKind::BadGrade,
        format!(
            "새 학년은 {}~{} 만 쓸 수 있습니다.",
            rules::MIN_GRADE,
            rules::MAX_GRADE
        ),
    );
    let mut unusual = Bag::new(
        ProblemKind::UnusualGrade,
        "한 학년 올라가는 것과 다른 새 학년이 적혀 있습니다. 맞는지 확인해 주세요.",
    );
    let mut no_next = Bag::new(
        ProblemKind::NoNextEnrollment,
        "졸업에서 뺐지만 다음 학년도 배정도 없습니다. 전환 뒤에 직접 학적을 만들어 주세요.",
    );

    struct Pending<'a> {
        seat: &'a repo::SeatRow,
        grade: i32,
        class_name: String,
        class_no: Option<i32>,
    }
    let mut pending: Vec<Pending> = Vec::new();
    let mut leaves: Vec<Leave> = Vec::new();
    let mut rows: Vec<PlanRow> = Vec::new();
    let mut skipped: Vec<&repo::SeatRow> = Vec::new();

    for (i, seat) in seats.iter().enumerate() {
        let assign = matched.by_seat.get(&i);
        // 6학년에게 새 반이 적혀 있으면 사람이 일부러 남긴 것이다 (유예 등) — 졸업시키지 않는다
        let has_new_class = assign
            .and_then(|r| r.new_class.as_deref())
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false);
        // 기본 처리는 학년이 정한다 — 1~5학년은 진급, 6학년은 졸업
        let graduating = rules::outcome(seat.grade) == Outcome::Graduate
            && !excluded.contains(&seat.student_id)
            && !has_new_class;

        if graduating {
            leaves.push(Leave {
                student_id: seat.student_id,
                grade: seat.grade,
                class_name: seat.class_name.clone(),
                class_no: seat.class_no,
            });
            rows.push(PlanRow {
                student_id: seat.student_id,
                name: seat.name.clone(),
                current: seat.seat_label(),
                next: "졸업".into(),
                fate: "GRADUATE",
            });
            continue;
        }

        let Some(row) = assign.filter(|_| has_new_class || seat.grade != rules::MAX_GRADE) else {
            // 6학년인데 졸업에서 뺐고 배정도 없으면 아무것도 만들지 않는다 (알리기만)
            if seat.grade == rules::MAX_GRADE {
                no_next.students.push(person(seat, None));
            } else {
                no_assign.students.push(person(seat, None));
            }
            skipped.push(seat);
            rows.push(PlanRow {
                student_id: seat.student_id,
                name: seat.name.clone(),
                current: seat.seat_label(),
                next: "없음".into(),
                fate: "NONE",
            });
            continue;
        };

        let grade = row
            .new_grade
            .or_else(|| rules::next_grade(seat.grade))
            .unwrap_or(seat.grade);
        if !rules::is_valid_grade(grade) {
            bad_grade
                .students
                .push(person(seat, Some(format!("파일: {grade}학년"))));
            skipped.push(seat);
            rows.push(PlanRow {
                student_id: seat.student_id,
                name: seat.name.clone(),
                current: seat.seat_label(),
                next: "없음".into(),
                fate: "NONE",
            });
            continue;
        }
        if Some(grade) != rules::next_grade(seat.grade) {
            unusual
                .students
                .push(person(seat, Some(format!("새 학년 {grade}"))));
        }

        let class_name = row
            .new_class
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty());
        let Some(class_name) = class_name else {
            no_class.students.push(person(seat, None));
            skipped.push(seat);
            rows.push(PlanRow {
                student_id: seat.student_id,
                name: seat.name.clone(),
                current: seat.seat_label(),
                next: "없음".into(),
                fate: "NONE",
            });
            continue;
        };

        pending.push(Pending {
            seat,
            grade,
            class_name: class_name.to_string(),
            class_no: row.new_no,
        });
    }

    // ---- 반마다 번호를 본다 ----
    let mut order: Vec<(i32, String)> = Vec::new();
    for p in &pending {
        let key = (p.grade, p.class_name.clone());
        if !order.contains(&key) {
            order.push(key);
        }
    }
    order.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| class_order(&a.1).cmp(&class_order(&b.1))));

    let mut partial = Bag::new(
        ProblemKind::PartialNumbers,
        "새 번호가 일부 학생에게만 입력되어 있습니다. 반 전체를 채우거나 모두 비워 주세요 — 빈 자리를 프로그램이 채우면 의도한 배치가 깨집니다.",
    );
    let mut dup_no = Bag::new(
        ProblemKind::NumberDup,
        "같은 반에 같은 번호가 둘 이상 있습니다.",
    );

    let mut moves: Vec<Move> = Vec::new();
    let mut classes: Vec<ClassPreview> = Vec::new();

    for (grade, class_name) in &order {
        let mates: Vec<&Pending> = pending
            .iter()
            .filter(|p| p.grade == *grade && &p.class_name == class_name)
            .collect();
        let numbers: Vec<Option<i32>> = mates.iter().map(|p| p.class_no).collect();

        let numbered: Vec<(i64, Option<i32>)> = match rules::number_state(&numbers) {
            NumberState::AllGiven => {
                for no in rules::duplicate_numbers(&numbers) {
                    for p in mates.iter().filter(|p| p.class_no == Some(no)) {
                        dup_no
                            .students
                            .push(person(p.seat, Some(format!("{grade}-{class_name}-{no}"))));
                    }
                }
                mates.iter().map(|p| (p.seat.student_id, p.class_no)).collect()
            }
            NumberState::AllEmpty => {
                // 이름 가나다순으로 1~N — 명단 정렬과 같은 규칙이다
                let names: Vec<(i64, String)> = mates
                    .iter()
                    .map(|p| (p.seat.student_id, p.seat.name.clone()))
                    .collect();
                rules::numbers_by_name(&names)
                    .into_iter()
                    .map(|(id, no)| (id, Some(no)))
                    .collect()
            }
            NumberState::Partial => {
                for p in &mates {
                    partial.students.push(person(
                        p.seat,
                        Some(match p.class_no {
                            Some(n) => format!("{n}번"),
                            None => "번호 없음".into(),
                        }),
                    ));
                }
                mates.iter().map(|p| (p.seat.student_id, p.class_no)).collect()
            }
        };
        let no_of: HashMap<i64, Option<i32>> = numbered.into_iter().collect();

        let (mut male, mut female, mut unknown_g) = (0i64, 0i64, 0i64);
        for p in &mates {
            match p.seat.gender.as_deref() {
                Some("M") => male += 1,
                Some("F") => female += 1,
                _ => unknown_g += 1,
            }
            let class_no = no_of.get(&p.seat.student_id).copied().flatten();
            moves.push(Move {
                student_id: p.seat.student_id,
                grade: *grade,
                class_name: class_name.clone(),
                class_no,
            });
            rows.push(PlanRow {
                student_id: p.seat.student_id,
                name: p.seat.name.clone(),
                current: p.seat.seat_label(),
                next: match class_no {
                    Some(n) => format!("{}-{n}", label::class_label(*grade, Some(class_name))),
                    None => label::class_label(*grade, Some(class_name)),
                },
                fate: "PROMOTE",
            });
        }

        classes.push(ClassPreview {
            grade: *grade,
            class_name: Some(class_name.clone()),
            class_label: label::class_label(*grade, Some(class_name)),
            male,
            female,
            unknown: unknown_g,
            total: mates.len() as i64,
        });
    }

    for bag in [no_assign, no_class, bad_grade, partial, dup_no, unusual, no_next] {
        if !bag.students.is_empty() {
            bags.push(bag);
        }
    }

    // ---- 학년별 요약 ----
    let mut by_grade: Vec<GradeMove> = Vec::new();
    for g in rules::MIN_GRADE..=rules::MAX_GRADE {
        let ids: Vec<i64> = seats
            .iter()
            .filter(|s| s.grade == g)
            .map(|s| s.student_id)
            .collect();
        let promoted = moves.iter().filter(|m| ids.contains(&m.student_id)).count();
        if promoted > 0 {
            let to = moves
                .iter()
                .find(|m| ids.contains(&m.student_id))
                .map(|m| m.grade);
            by_grade.push(GradeMove {
                from_grade: g,
                to_grade: to,
                label: format!("{g}학년 → {}학년", to.unwrap_or(g)),
                count: promoted,
            });
        }
        let left = leaves.iter().filter(|l| l.grade == g).count();
        if left > 0 {
            by_grade.push(GradeMove {
                from_grade: g,
                to_grade: None,
                label: format!("{g}학년 → 졸업"),
                count: left,
            });
        }
    }

    // 학생별 표는 명단 차례(현재 학적)로 되돌려 놓는다
    let place: HashMap<i64, usize> = seats
        .iter()
        .enumerate()
        .map(|(i, s)| (s.student_id, i))
        .collect();
    rows.sort_by_key(|r| place.get(&r.student_id).copied().unwrap_or(usize::MAX));

    let problems: Vec<Problem> = bags
        .into_iter()
        .map(|b| Problem {
            kind: b.kind,
            title: b.kind.label().to_string(),
            message: b.message,
            blocking: b.kind.blocking(),
            students: b.students,
        })
        .collect();
    let blocked = problems.iter().any(|p| p.blocking);

    Ok(Plan {
        from_year,
        to_year,
        target_count: seats.len(),
        promote_count: moves.len(),
        graduate_count: leaves.len(),
        skip_count: skipped.len(),
        existing_count: target.enrollments,
        rows,
        by_grade,
        classes,
        problems,
        state_key,
        blocked,
        assign_file: assigns.map(|a| a.file_name.clone()),
        assign_rows: assigns.map(|a| a.rows.len()).unwrap_or(0),
        moves,
        leaves,
    })
}

/// 반 정렬 — 숫자 반을 먼저, 그다음 이름 반. 명단과 같은 차례다.
fn class_order(name: &str) -> (u8, i64, String) {
    match name.trim().parse::<i64>() {
        Ok(n) => (0, n, String::new()),
        Err(_) => (1, 0, name.trim().to_string()),
    }
}

// ---------------------------------------------------------------
// 적용
// ---------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub from_year: i32,
    pub to_year: i32,
    pub promoted: usize,
    pub graduated: usize,
    pub skipped: usize,
    /// 새 학년도 학생에게 열린 확인 필요 건수
    pub issues: i64,
}

/// 계획대로 만든다. **한 트랜잭션 안에서 불러야 한다.**
///
/// 부르기 전에 `build` 로 계획을 다시 세워 `state_key` 를 견주는 것은 부르는 쪽 몫이다.
pub fn apply(
    c: &Connection,
    plan: &Plan,
    today: NaiveDate,
    mut on_progress: impl FnMut(&str, &str, usize, usize),
) -> AppResult<Summary> {
    if plan.blocked {
        return Err(AppError::invalid(
            "먼저 확인해야 할 것이 남아 있습니다. 전환을 시작하지 않았습니다.",
        ));
    }

    let total = plan.moves.len() + plan.leaves.len();
    let step = (total / 20).max(1);

    on_progress("PROMOTE", "다음 학년도 학적 만들기", 0, total);
    for (i, m) in plan.moves.iter().enumerate() {
        repo::promote(
            c,
            m.student_id,
            plan.to_year,
            m.grade,
            Some(&m.class_name),
            m.class_no,
        )?;
        if i % step == 0 {
            on_progress("PROMOTE", "다음 학년도 학적 만들기", i, total);
        }
    }

    on_progress("GRADUATE", "졸업 처리", plan.moves.len(), total);
    for l in &plan.leaves {
        repo::graduate(
            c,
            l.student_id,
            plan.from_year,
            l.grade,
            l.class_name.as_deref(),
            l.class_no,
            today,
        )?;
    }

    // 새 학년도 기준으로 확인 필요를 다시 따진다 (반 미정·번호 중복 등)
    on_progress("ISSUE", "확인 필요 정리", 0, plan.moves.len());
    for (i, m) in plan.moves.iter().enumerate() {
        student::sync_issues(c, m.student_id, plan.to_year, today)?;
        if i % step == 0 {
            on_progress("ISSUE", "확인 필요 정리", i, plan.moves.len());
        }
    }
    // 졸업생은 명단에서 빠지므로 그 학년도 표시를 다시 맞춘다
    for l in &plan.leaves {
        student::sync_issues(c, l.student_id, plan.from_year, today)?;
    }

    let issues: i64 = c.query_row(
        "SELECT COUNT(*) FROM issues i
           JOIN enrollments e ON e.student_id = i.student_id AND e.school_year = ?1
          WHERE i.status = 'OPEN'",
        [plan.to_year],
        |r| r.get(0),
    )?;

    let summary = Summary {
        from_year: plan.from_year,
        to_year: plan.to_year,
        promoted: plan.moves.len(),
        graduated: plan.leaves.len(),
        skipped: plan.skip_count,
        issues,
    };
    // 기록에도 개인정보는 넣지 않는다 — 학년도와 인원뿐이다
    repo::record(
        c,
        plan.from_year,
        plan.to_year,
        &serde_json::json!({
            "promoted": summary.promoted,
            "graduated": summary.graduated,
            "skipped": summary.skipped,
        })
        .to_string(),
    )?;
    on_progress("FINISH", "마무리", total, total);
    Ok(summary)
}

/// 졸업 대상으로 제안할 학생 (원본 학년도 6학년 재학생).
pub fn graduation_candidates(
    c: &Connection,
    from_year: i32,
    today: NaiveDate,
) -> AppResult<Vec<Person>> {
    Ok(repo::seats(c, from_year, today)?
        .iter()
        .filter(|s| s.grade == rules::MAX_GRADE)
        .map(|s| person(s, None))
        .collect())
}

#[cfg(test)]
#[path = "transition_tests.rs"]
mod transition_tests;
