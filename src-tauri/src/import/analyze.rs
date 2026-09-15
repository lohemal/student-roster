//! 엑셀 줄을 기존 자료와 견주어 **무엇을 할지** 정한다. 아무것도 저장하지 않는다.
//!
//! 가장 위험한 실수는 **다른 학생의 정상 자료를 덮어쓰는 것**이다. 그래서
//!   * 확실히 같은 학생일 때만 갱신 대상으로 본다.
//!   * 조금이라도 애매하면 '중복 의심'으로 빼 두고 사람이 판단하게 한다.
//!   * 엑셀 빈칸으로 기존 값을 지우지 않는다.
//!
//! 적용(`apply`)도 이 판정을 **트랜잭션 안에서 다시** 돌린다. 미리보기와 실제 결과가
//! 어긋날 여지를 없애기 위해서다.

use std::collections::HashMap;

use chrono::NaiveDate;
use rusqlite::{params, Connection};
use serde::Serialize;

use super::row::{self, ParsedRow, RowWarning};
use crate::domain::{birth, phone};
use crate::error::AppResult;
use crate::repo::student::StudentInput;

/// 이 줄을 어떻게 할 것인가.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    /// 새 학생으로 등록
    Add,
    /// 기존 학생 갱신
    Update,
    /// 같은 학생이고 바뀐 것도 없음
    Unchanged,
    /// 같은 학생인지 확실하지 않음 — 자동으로 건드리지 않는다
    Ambiguous,
    /// 이 줄은 가져올 수 없음 (이름·학년 없음 등)
    Blocked,
}

impl Action {
    pub fn label(self) -> &'static str {
        match self {
            Action::Add => "신규 추가",
            Action::Update => "기존 학생 갱신",
            Action::Unchanged => "변경 없음",
            Action::Ambiguous => "중복 의심",
            Action::Blocked => "가져올 수 없음",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldChange {
    pub field: String,
    pub before: String,
    pub after: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub student_id: i64,
    pub name: String,
    pub birth: String,
    pub where_at: String,
}

/// 분석한 줄 하나. 화면이 그대로 그린다.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RowPlan {
    pub excel_row: usize,
    pub action: Action,
    pub action_label: String,
    pub name: String,
    pub where_at: String,
    pub birth: String,
    /// 갱신일 때 어떤 학생을 고치는지
    pub student_id: Option<i64>,
    pub changes: Vec<FieldChange>,
    /// 중복 의심·가져올 수 없음의 이유
    pub reason: Option<String>,
    pub candidates: Vec<Candidate>,
    /// 저장 뒤 '확인 필요'가 될 것으로 보이는 것
    pub warnings: Vec<RowWarning>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub total: usize,
    pub add: usize,
    pub update: usize,
    pub unchanged: usize,
    pub ambiguous: usize,
    pub blocked: usize,
    /// 확인 필요가 예상되는 줄 수
    pub warned: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeResult {
    pub summary: Summary,
    pub rows: Vec<RowPlan>,
}

// ---------------------------------------------------------------
// 기존 학생 찾기
// ---------------------------------------------------------------

/// 갱신 때 견줄 기존 값. 한 번의 조회로 학생과 그 학년도 학적을 함께 가져온다.
#[derive(Debug, Clone)]
pub struct Existing {
    pub id: i64,
    pub name: String,
    pub gender: Option<String>,
    pub birth_raw: Option<String>,
    pub birth_date: Option<String>,
    pub address_raw: Option<String>,
    pub father_name: Option<String>,
    pub mother_name: Option<String>,
    pub father_phone: Option<String>,
    pub mother_phone: Option<String>,
    pub primary_phone: Option<String>,
    pub note: Option<String>,
    /// 대상 학년도 학적이 있는가
    pub has_enrollment: bool,
    pub grade: Option<i32>,
    pub class_name: Option<String>,
    pub class_no: Option<i32>,
}

pub fn load_existing(c: &Connection, id: i64, year: i32) -> AppResult<Existing> {
    Ok(c.query_row(
        "SELECT s.id, s.name, s.gender, s.birth_raw, s.birth_date, s.address_raw,
                s.father_name, s.mother_name, s.father_phone, s.mother_phone,
                s.primary_phone, s.note,
                e.grade, e.class_name, e.class_no
           FROM students s
           LEFT JOIN enrollments e ON e.student_id = s.id AND e.school_year = ?2
          WHERE s.id = ?1",
        params![id, year],
        |r| {
            let grade: Option<i32> = r.get(12)?;
            Ok(Existing {
                id: r.get(0)?,
                name: r.get(1)?,
                gender: r.get(2)?,
                birth_raw: r.get(3)?,
                birth_date: r.get(4)?,
                address_raw: r.get(5)?,
                father_name: r.get(6)?,
                mother_name: r.get(7)?,
                father_phone: r.get(8)?,
                mother_phone: r.get(9)?,
                primary_phone: r.get(10)?,
                note: r.get(11)?,
                has_enrollment: grade.is_some(),
                grade,
                class_name: r.get(13)?,
                class_no: r.get(14)?,
            })
        },
    )?)
}

fn ids_by_name_and_birth(c: &Connection, name: &str, birth: &str) -> AppResult<Vec<i64>> {
    let mut st = c.prepare_cached("SELECT id FROM students WHERE name = ?1 AND birth_date = ?2")?;
    let ids: Vec<i64> = st
        .query_map(params![name, birth], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(ids)
}

fn ids_by_seat(
    c: &Connection,
    year: i32,
    grade: i32,
    class_name: &str,
    class_no: i32,
    name: &str,
) -> AppResult<Vec<i64>> {
    let mut st = c.prepare_cached(
        "SELECT s.id FROM students s
           JOIN enrollments e ON e.student_id = s.id
          WHERE e.school_year = ?1 AND e.grade = ?2 AND e.class_name = ?3
            AND e.class_no = ?4 AND s.name = ?5",
    )?;
    let ids: Vec<i64> = st
        .query_map(params![year, grade, class_name, class_no, name], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(ids)
}

fn ids_by_name(c: &Connection, name: &str) -> AppResult<Vec<i64>> {
    let mut st = c.prepare_cached("SELECT id FROM students WHERE name = ?1")?;
    let ids: Vec<i64> = st
        .query_map([name], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(ids)
}

fn brief(c: &Connection, id: i64, year: i32) -> AppResult<Candidate> {
    let e = load_existing(c, id, year)?;
    Ok(Candidate {
        student_id: e.id,
        name: e.name,
        birth: e
            .birth_date
            .as_deref()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
            .map(birth::format_display)
            .unwrap_or_else(|| "생년월일 없음".into()),
        where_at: match (e.grade, e.class_name.as_deref(), e.class_no) {
            (Some(g), c, n) => format!(
                "{}{}",
                crate::domain::label::class_label(g, c),
                n.map(|x| format!(" {x}번")).unwrap_or_default()
            ),
            _ => format!("{year}학년도 학적 없음"),
        },
    })
}

/// 같은 학생을 찾은 결과.
enum Found {
    None,
    One(i64),
    Many(Vec<i64>, String),
}

/// 같은 학생인지 가린다.
///
/// 1. **이름 + 생년월일**이 둘 다 있고 딱 하나 걸리면 같은 학생으로 본다.
/// 2. 생년월일이 없으면 **그 학년도의 같은 자리(학년·반·번호) + 같은 이름**으로 가린다.
/// 3. 생년월일은 있는데 걸리는 학생이 없고, 같은 자리에 같은 이름이 있으면
///    **생년월일이 달라진 것**이므로 자동으로 정하지 않는다 (중복 의심).
/// 4. 그 밖에 이름만 같은 학생이 있으면 동명이인일 수 있으므로 자동 갱신하지 않는다.
fn find_same_student(c: &Connection, row: &ParsedRow, year: i32) -> AppResult<Found> {
    let seat = match (row.grade, row.class_name.as_deref(), row.class_no) {
        (Some(g), Some(cn), Some(no)) => Some(ids_by_seat(c, year, g, cn, no, &row.name)?),
        _ => None,
    };

    if let Some(bd) = row.birth_date {
        let by_birth = ids_by_name_and_birth(c, &row.name, &birth::format_iso(bd))?;
        return Ok(match by_birth.len() {
            1 => Found::One(by_birth[0]),
            0 => match seat {
                Some(s) if s.len() == 1 => Found::Many(
                    s,
                    "같은 학년·반·번호에 이름이 같은 학생이 있지만 생년월일이 다릅니다.".into(),
                ),
                Some(s) if s.len() > 1 => {
                    Found::Many(s, "같은 자리에 이름이 같은 학생이 여럿입니다.".into())
                }
                // 이름이 같아도 생년월일이 다르면 동명이인이다 — 새 학생으로 본다
                _ => Found::None,
            },
            _ => Found::Many(
                by_birth,
                "이름과 생년월일이 같은 학생이 이미 여럿 있습니다.".into(),
            ),
        });
    }

    // 생년월일이 없다 — 자리로 가린다
    if let Some(s) = seat {
        match s.len() {
            1 => return Ok(Found::One(s[0])),
            0 => {}
            _ => {
                return Ok(Found::Many(
                    s,
                    "같은 자리에 이름이 같은 학생이 여럿입니다.".into(),
                ))
            }
        }
    }

    let by_name = ids_by_name(c, &row.name)?;
    Ok(if by_name.is_empty() {
        Found::None
    } else {
        Found::Many(
            by_name,
            "이름이 같은 학생이 이미 있는데, 생년월일이 없어 같은 학생인지 가릴 수 없습니다.".into(),
        )
    })
}

// ---------------------------------------------------------------
// 무엇이 바뀌는지 / 무엇을 저장할지
// ---------------------------------------------------------------

/// 값을 견주는 방법. 겉모양이 달라도 같은 값이면 '바뀜'으로 세지 않는다.
#[derive(Clone, Copy)]
enum Kind {
    Text,
    Phone,
    Birth,
}

fn same(kind: Kind, before: Option<&str>, after: &str) -> bool {
    let b = before.unwrap_or("").trim();
    match kind {
        Kind::Text => b == after.trim(),
        // 010-1234-5678 과 01012345678 은 같은 번호다
        Kind::Phone => phone::digits(b) == phone::digits(after),
        Kind::Birth => b == after.trim(),
    }
}

/// 엑셀 값으로 바뀌는 것과, 저장할 최종 값을 **한 번에** 만든다.
///
/// 규칙: 엑셀에 값이 있을 때만 바꾼다. **빈칸은 기존 값을 지우지 않는다.**
/// (나중에 '빈칸도 반영' 옵션이 필요해지면 이 함수에 갈래를 하나 더한다.)
pub fn merge(existing: &Existing, row: &ParsedRow, year: i32) -> (StudentInput, Vec<FieldChange>) {
    let mut changes = Vec::new();

    // (항목 이름, 비교 방법, 기존 값, 엑셀 값)
    let mut pick = |label: &str, kind: Kind, before: Option<&str>, after: Option<&str>| -> Option<String> {
        match after.map(str::trim).filter(|s| !s.is_empty()) {
            None => before.map(str::to_string), // 빈칸 — 기존 값 유지
            Some(v) => {
                if !same(kind, before, v) {
                    changes.push(FieldChange {
                        field: label.to_string(),
                        before: before.unwrap_or("").to_string(),
                        after: v.to_string(),
                    });
                }
                Some(v.to_string())
            }
        }
    };

    let birth_raw = pick(
        "생년월일",
        Kind::Birth,
        existing.birth_raw.as_deref(),
        row.birth_raw.as_deref(),
    );
    let gender = pick("성별", Kind::Text, existing.gender.as_deref(), row.gender.as_deref());
    let address = pick(
        "주소",
        Kind::Text,
        existing.address_raw.as_deref(),
        row.address.as_deref(),
    );
    let father_name = pick(
        "부 성명",
        Kind::Text,
        existing.father_name.as_deref(),
        row.father_name.as_deref(),
    );
    let mother_name = pick(
        "모 성명",
        Kind::Text,
        existing.mother_name.as_deref(),
        row.mother_name.as_deref(),
    );
    let father_phone = pick(
        "부 연락처",
        Kind::Phone,
        existing.father_phone.as_deref(),
        row.father_phone.as_deref(),
    );
    let mother_phone = pick(
        "모 연락처",
        Kind::Phone,
        existing.mother_phone.as_deref(),
        row.mother_phone.as_deref(),
    );
    let primary_phone = pick(
        "주보호자 연락처",
        Kind::Phone,
        existing.primary_phone.as_deref(),
        row.primary_phone.as_deref(),
    );
    let note = pick("비고", Kind::Text, existing.note.as_deref(), row.note.as_deref());

    // 학적 — 학년은 엑셀 값이 있으면 그것, 없으면 기존 것
    let grade = row.grade.or(existing.grade).unwrap_or(1);
    if existing.has_enrollment && Some(grade) != existing.grade {
        changes.push(FieldChange {
            field: "학년".into(),
            before: existing.grade.map(|g| format!("{g}학년")).unwrap_or_default(),
            after: format!("{grade}학년"),
        });
    }

    let class_name = match row.class_name.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        None => existing.class_name.clone(),
        Some(v) => {
            if existing.has_enrollment && !same(Kind::Text, existing.class_name.as_deref(), v) {
                changes.push(FieldChange {
                    field: "반".into(),
                    before: existing.class_name.clone().unwrap_or_default(),
                    after: v.to_string(),
                });
            }
            Some(v.to_string())
        }
    };

    let class_no = match row.class_no {
        None => existing.class_no,
        Some(v) => {
            if existing.has_enrollment && existing.class_no != Some(v) {
                changes.push(FieldChange {
                    field: "번호".into(),
                    before: existing.class_no.map(|n| format!("{n}번")).unwrap_or_default(),
                    after: format!("{v}번"),
                });
            }
            Some(v)
        }
    };

    let input = StudentInput {
        name: existing.name.clone(), // 이름은 매칭 기준이므로 가져오기로 바꾸지 않는다
        gender,
        birth_raw,
        address_raw: address,
        father_name,
        mother_name,
        father_phone,
        mother_phone,
        primary_phone,
        note,
        school_year: year,
        grade,
        class_name,
        class_no,
    };
    (input, changes)
}

// ---------------------------------------------------------------
// 분석
// ---------------------------------------------------------------

fn where_of(row: &ParsedRow) -> String {
    match row.grade {
        Some(g) => format!(
            "{}{}",
            crate::domain::label::class_label(g, row.class_name.as_deref()),
            row.class_no.map(|n| format!(" {n}번")).unwrap_or_default()
        ),
        None => "학년 미상".to_string(),
    }
}

/// 전체 줄을 분석한다. **DB를 바꾸지 않는다.**
///
/// `on_progress(done, total)` 로 진행 상황을 알린다.
pub fn run(
    c: &Connection,
    rows: &[ParsedRow],
    year: i32,
    mut on_progress: impl FnMut(usize, usize),
) -> AppResult<AnalyzeResult> {
    let total = rows.len();
    let mut plans = Vec::with_capacity(total);
    let mut summary = Summary {
        total,
        ..Default::default()
    };

    // 한 파일 안에서 같은 학생이 두 번 나오는 경우를 잡는다
    let mut claimed: HashMap<i64, usize> = HashMap::new();
    let mut new_keys: HashMap<String, usize> = HashMap::new();

    for (i, row) in rows.iter().enumerate() {
        let warnings = row::warnings(row);
        let mut plan = RowPlan {
            excel_row: row.excel_row,
            action: Action::Add,
            action_label: String::new(),
            name: row.name.clone(),
            where_at: where_of(row),
            birth: row
                .birth_date
                .map(birth::format_display)
                .or_else(|| row.birth_raw.clone())
                .unwrap_or_default(),
            student_id: None,
            changes: Vec::new(),
            reason: None,
            candidates: Vec::new(),
            warnings,
        }
        .with_label();

        if let Some(b) = &row.blocker {
            plan.action = Action::Blocked;
            plan.reason = Some(b.clone());
        } else {
            match find_same_student(c, row, year)? {
                Found::None => {
                    // 이 파일 안에서 같은 사람이 이미 나왔는가
                    let key = format!(
                        "{}|{}",
                        row.name,
                        row.birth_date.map(birth::format_iso).unwrap_or_default()
                    );
                    match new_keys.get(&key) {
                        Some(first) if row.birth_date.is_some() => {
                            plan.action = Action::Ambiguous;
                            plan.reason = Some(format!(
                                "이 파일의 {}번째 줄과 이름·생년월일이 같습니다.",
                                first
                            ));
                        }
                        _ => {
                            new_keys.insert(key, row.excel_row);
                            plan.action = Action::Add;
                        }
                    }
                }
                Found::One(id) => {
                    if let Some(first) = claimed.get(&id) {
                        plan.action = Action::Ambiguous;
                        plan.reason = Some(format!(
                            "이 파일의 {}번째 줄과 같은 학생을 가리킵니다.",
                            first
                        ));
                        plan.candidates = vec![brief(c, id, year)?];
                    } else {
                        claimed.insert(id, row.excel_row);
                        let existing = load_existing(c, id, year)?;
                        let (_, changes) = merge(&existing, row, year);
                        plan.student_id = Some(id);
                        if changes.is_empty() && existing.has_enrollment {
                            plan.action = Action::Unchanged;
                        } else {
                            plan.action = Action::Update;
                            if !existing.has_enrollment {
                                plan.changes.push(FieldChange {
                                    field: "학적".into(),
                                    before: format!("{year}학년도 학적 없음"),
                                    after: where_of(row),
                                });
                            }
                            plan.changes.extend(changes);
                        }
                    }
                }
                Found::Many(ids, why) => {
                    plan.action = Action::Ambiguous;
                    plan.reason = Some(why);
                    for id in ids.iter().take(5) {
                        plan.candidates.push(brief(c, *id, year)?);
                    }
                }
            }
        }

        plan = plan.with_label();
        match plan.action {
            Action::Add => summary.add += 1,
            Action::Update => summary.update += 1,
            Action::Unchanged => summary.unchanged += 1,
            Action::Ambiguous => summary.ambiguous += 1,
            Action::Blocked => summary.blocked += 1,
        }
        if !plan.warnings.is_empty() && plan.action != Action::Blocked {
            summary.warned += 1;
        }
        plans.push(plan);

        if i % progress_step(total) == 0 {
            on_progress(i, total);
        }
    }
    on_progress(total, total);

    Ok(AnalyzeResult {
        summary,
        rows: plans,
    })
}

impl RowPlan {
    fn with_label(mut self) -> Self {
        self.action_label = self.action.label().to_string();
        self
    }
}

/// 진행 상황을 얼마나 자주 알릴지. 줄마다 알리면 화면이 그것만 그리다 만다.
pub fn progress_step(total: usize) -> usize {
    (total / 100).max(1)
}

#[cfg(test)]
#[path = "analyze_tests.rs"]
mod analyze_tests;
