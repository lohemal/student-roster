//! 본교 형제 관계 저장소.
//!
//! 판정 규칙은 `domain::sibling` 에 있다. 여기서는 학생을 읽어 넘기고,
//! 나온 쌍을 `sibling_links` 에 담고, 사용자의 결정을 지키는 일을 한다.
//!
//! 지키는 것
//!   * **사용자 결정을 자동화가 뒤집지 않는다.** 한 번 '형제 아님'(REJECTED)이라고
//!     한 쌍은 다시 훑어도 후보로 되살아나지 않는다. '형제'(CONFIRMED)도 그대로 둔다.
//!   * 형제 관계와 보호자 정보 일치는 **다른 문제**다. 연락처가 달라도 형제일 수 있다.
//!   * 보호자 정보를 저절로 베끼지 않는다. 사람이 고른 항목만, 그것도 **빈칸에만** 채운다.
//!   * 표시 글(`1-나리 홍길동`)은 저장하지 않는다. 진급하면 저절로 바뀌어야 하므로
//!     볼 때마다 지금 학적으로 만든다.

use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::domain::label;
use crate::domain::sibling::{self, Comparison, Entry, Field, Guardians};
use crate::domain::enroll;
use crate::error::{AppError, AppResult};

// ---------------------------------------------------------------
// 읽기
// ---------------------------------------------------------------

/// 그 학년도 재학생(재학·전입)의 보호자 정보.
///
/// 전출·졸업한 학생은 훑기에서 뺀다. 관계 자체는 학생 번호로 남으므로
/// 지난 기록은 그대로 볼 수 있다.
///
/// **아직 오지 않은 전입생은 넣는다**(`enrolled_sql`). 후보 판정은 '오늘 교실에
/// 있는가' 가 아니라 '이 학년도 명단에 이름이 있는가' 이고, 빼 두면 그 학생이 오는
/// 날 아무도 다시 훑어 주지 않아 후보가 영영 생기지 않는다. '본교 형제 수' 에
/// 드는지는 `together_now` 가 따로 가린다.
pub fn active_entries(
    c: &Connection,
    school_year: i32,
    asof: NaiveDate,
) -> AppResult<Vec<Entry>> {
    let active = enroll::enrolled_sql("e.", asof);
    let mut st = c.prepare(&format!(
        "SELECT s.id, s.father_name, s.mother_name, s.father_phone, s.mother_phone
           FROM students s
           JOIN enrollments e ON e.student_id = s.id
          WHERE e.school_year = ?1 AND {active}
            AND NOT EXISTS (SELECT 1 FROM graduations g WHERE g.student_id = s.id)
          ORDER BY s.id",
    ))?;
    let rows = st
        .query_map([school_year], |r| {
            Ok(Entry {
                student_id: r.get(0)?,
                guardians: Guardians {
                    father_name: r.get(1)?,
                    mother_name: r.get(2)?,
                    father_phone: r.get(3)?,
                    mother_phone: r.get(4)?,
                },
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

pub fn guardians_of(c: &Connection, student_id: i64) -> AppResult<Guardians> {
    Ok(c.query_row(
        "SELECT father_name, mother_name, father_phone, mother_phone
           FROM students WHERE id = ?1",
        [student_id],
        |r| {
            Ok(Guardians {
                father_name: r.get(0)?,
                mother_name: r.get(1)?,
                father_phone: r.get(2)?,
                mother_phone: r.get(3)?,
            })
        },
    )?)
}

/// 저장된 관계 한 줄.
#[derive(Debug, Clone)]
pub struct Link {
    pub id: i64,
    pub student_a: i64,
    pub student_b: i64,
    pub status: String,
}

impl Link {
    /// 이 관계에서 `me` 의 상대.
    pub fn other(&self, me: i64) -> i64 {
        if self.student_a == me {
            self.student_b
        } else {
            self.student_a
        }
    }
}

/// 한 학생이 걸려 있는 모든 관계.
pub fn links_of(c: &Connection, student_id: i64) -> AppResult<Vec<Link>> {
    let mut st = c.prepare_cached(
        "SELECT id, student_a, student_b, status
           FROM sibling_links
          WHERE student_a = ?1 OR student_b = ?1
          ORDER BY id",
    )?;
    let rows: Vec<Link> = st
        .query_map([student_id], |r| {
            Ok(Link {
                id: r.get(0)?,
                student_a: r.get(1)?,
                student_b: r.get(2)?,
                status: r.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// 이 학생과 **형제로 확정된** 학생 번호들. 보호자 정보를 함께 다시 볼 때 쓴다.
pub fn confirmed_partners(c: &Connection, student_id: i64) -> AppResult<Vec<i64>> {
    Ok(links_of(c, student_id)?
        .into_iter()
        .filter(|l| l.status == "CONFIRMED")
        .map(|l| l.other(student_id))
        .collect())
}

fn find_link(c: &Connection, a: i64, b: i64) -> AppResult<Option<Link>> {
    let (lo, hi) = if a < b { (a, b) } else { (b, a) };
    Ok(c.query_row(
        "SELECT id, student_a, student_b, status
           FROM sibling_links WHERE student_a = ?1 AND student_b = ?2",
        [lo, hi],
        |r| {
            Ok(Link {
                id: r.get(0)?,
                student_a: r.get(1)?,
                student_b: r.get(2)?,
                status: r.get(3)?,
            })
        },
    )
    .optional()?)
}

// ---------------------------------------------------------------
// 화면에 보여 줄 모양
// ---------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldView {
    pub key: String,
    pub label: String,
    /// 이 학생의 값
    pub mine: Option<String>,
    /// 형제의 값
    pub theirs: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SiblingView {
    pub link_id: i64,
    pub student_id: i64,
    /// `1-나리 홍길동` — 지금 학적으로 그때그때 만든다
    pub label: String,
    pub status: String,
    pub status_label: String,
    /// 값이 있고 서로 같은 항목
    pub matched: Vec<FieldView>,
    /// 양쪽 다 값이 있는데 다른 항목
    pub conflicts: Vec<FieldView>,
    /// 형제에게는 있고 나에게는 없는 항목 — 가져올 수 있다
    pub fillable: Vec<FieldView>,
    /// 지금 함께 다니지 않으면 그 까닭 (`전출` / `지난 학년도`). 관계는 그대로 남는다.
    pub partner_note: Option<String>,
    pub found_at: String,
    pub decided_at: Option<String>,
}

pub fn status_label(status: &str) -> &'static str {
    match status {
        "CANDIDATE" => "후보",
        "CONFIRMED" => "확정",
        "REJECTED" => "형제 아님",
        _ => "알 수 없음",
    }
}

fn field_view(f: Field, mine: &Guardians, theirs: &Guardians) -> FieldView {
    FieldView {
        key: f.code().to_string(),
        label: f.label().to_string(),
        mine: mine.raw(f).map(str::to_string),
        theirs: theirs.raw(f).map(str::to_string),
    }
}

/// 학생이 쓰인 이름표. 지금 학년도 학적이 없으면 가장 최근 것을 쓴다.
/// 이름표 하나와 **명단 차례**를 매길 열쇠.
///
/// 형제를 여럿 늘어놓을 때 차례가 필요하다. 관계가 만들어진 차례대로 두면 같은
/// 학생을 볼 때마다 순서가 달라 보인다. 학생명단과 같은 규칙(학년 → 반 → 이름)으로
/// 늘어놓는다.
struct Labeled {
    order: (i32, (u8, i64, String), String),
    text: String,
}

fn labeled(c: &Connection, student_id: i64, school_year: i32) -> AppResult<Labeled> {
    let row: Option<(String, i32, Option<String>)> = c
        .query_row(
            "SELECT s.name, e.grade, e.class_name
               FROM students s
               JOIN enrollments e ON e.student_id = s.id
              WHERE s.id = ?1
              ORDER BY (e.school_year = ?2) DESC, e.school_year DESC
              LIMIT 1",
            params![student_id, school_year],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;

    Ok(match row {
        Some((name, grade, class_name)) => Labeled {
            order: (
                grade,
                label::class_sort_key(class_name.as_deref()),
                name.clone(),
            ),
            text: label::student_label(grade, class_name.as_deref(), &name),
        },
        // 학적이 하나도 없는 학생 (있을 수 없지만 이름만이라도 보여 준다)
        None => {
            let name: String = c
                .query_row("SELECT name FROM students WHERE id = ?1", [student_id], |r| {
                    r.get(0)
                })
                .optional()?
                .unwrap_or_else(|| "알 수 없는 학생".into());
            Labeled {
                order: (i32::MAX, (9, 0, String::new()), name.clone()),
                text: name,
            }
        }
    })
}

/// `1-나리 홍길동` — 지금 학적으로 그때그때 만든다.
pub fn student_label_of(c: &Connection, student_id: i64, school_year: i32) -> AppResult<String> {
    Ok(labeled(c, student_id, school_year)?.text)
}

/// 한 학생의 형제 관계를 화면에 보여 줄 모양으로.
pub fn list_for_student(
    c: &Connection,
    student_id: i64,
    school_year: i32,
    asof: NaiveDate,
) -> AppResult<Vec<SiblingView>> {
    let mine = guardians_of(c, student_id)?;
    let mut out = Vec::new();

    for link in links_of(c, student_id)? {
        let other = link.other(student_id);
        let theirs = guardians_of(c, other)?;
        let cmp = sibling::compare(&mine, &theirs);

        let (found_at, decided_at): (String, Option<String>) = c.query_row(
            "SELECT created_at, decided_at FROM sibling_links WHERE id = ?1",
            [link.id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;

        out.push(SiblingView {
            link_id: link.id,
            student_id: other,
            label: student_label_of(c, other, school_year)?,
            status_label: status_label(&link.status).to_string(),
            matched: cmp
                .matched
                .iter()
                .map(|f| field_view(*f, &mine, &theirs))
                .collect(),
            conflicts: cmp
                .conflicts
                .iter()
                .map(|f| field_view(*f, &mine, &theirs))
                .collect(),
            // 확정된 형제에게만 가져오기를 권한다
            fillable: if link.status == "CONFIRMED" {
                sibling::fillable(&mine, &theirs)
                    .iter()
                    .map(|f| field_view(*f, &mine, &theirs))
                    .collect()
            } else {
                Vec::new()
            },
            partner_note: partner_note(c, other, school_year, asof)?,
            status: link.status,
            found_at,
            decided_at,
        });
    }

    // 확정 → 후보 → 형제 아님 차례로
    out.sort_by_key(|v| match v.status.as_str() {
        "CONFIRMED" => 0,
        "CANDIDATE" => 1,
        _ => 2,
    });
    Ok(out)
}

/// 학생명단에 보여 줄 형제 표시.
///
/// **몇 명인지가 아니라 누구인지를 보여 준다.** 둘 이상이어도 `2명` 으로 줄이지
/// 않는다 — 명단을 보며 형제를 확인하는 일이 실제 업무이고, 수만 적혀 있으면
/// 학생 상세를 한 명씩 열어야 한다.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SiblingBrief {
    pub count: i64,
    /// `5-2 오정우` · `2-1 김하늘 · 4-2 김바다` — 지금 학적으로 그때그때 만든다
    pub text: String,
    /// 이름표 하나씩. 화면이 줄바꿈할 자리를 스스로 고를 수 있게 함께 준다.
    pub labels: Vec<String>,
}

/// 형제 이름표를 잇는 글자. 이름에 쉼표가 들어가도 헷갈리지 않는다.
pub const BRIEF_SEP: &str = " · ";

/// 그 학년도에 이 학생이 어떤 상태인지. 지금 학적이 없으면 None.
///
/// 형제가 전출했거나 지난 학년도 학생이면 표시를 달리해야 한다 — 관계는 그대로
/// 두되 **지금 본교에 함께 다니는 것은 아니라는 사실**을 알려 주기 위해서다.
type Seat = (String, Option<String>, Option<String>);

fn seat_in_year(c: &Connection, student_id: i64, school_year: i32) -> AppResult<Option<Seat>> {
    Ok(c.query_row(
        "SELECT status, transfer_in_date, transfer_out_date
           FROM enrollments WHERE student_id = ?1 AND school_year = ?2",
        params![student_id, school_year],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .optional()?)
}

/// 기준일에 본교에 함께 다니는 형제인가.
///
/// 상태만 보지 않는다 — 전입 예정 형제는 아직 세지 않고, 전출 예정 형제는
/// 그날까지 센다. 명단·통계와 같은 `enroll::active_on` 을 쓴다.
fn together_now(
    c: &Connection,
    student_id: i64,
    school_year: i32,
    asof: NaiveDate,
) -> AppResult<bool> {
    Ok(seat_in_year(c, student_id, school_year)?
        .map(|(status, in_date, out_date)| {
            enroll::active_on(&status, in_date.as_deref(), out_date.as_deref(), asof)
        })
        .unwrap_or(false))
}

/// 형제 이름표 뒤에 붙일 말. 함께 다니고 있으면 None.
/// 졸업을 먼저 본다 — 졸업생은 그 뒤 학년도에 학적이 없으므로 그냥 두면
/// '지난 학년도' 로 보인다. 왜 함께 다니지 않는지를 바로 말해야 한다.
fn partner_note(
    c: &Connection,
    student_id: i64,
    school_year: i32,
    asof: NaiveDate,
) -> AppResult<Option<String>> {
    let graduated: i64 = c.query_row(
        "SELECT COUNT(*) FROM graduations WHERE student_id = ?1 AND school_year <= ?2",
        params![student_id, school_year],
        |r| r.get(0),
    )?;
    if graduated > 0 {
        return Ok(Some("졸업".into()));
    }
    Ok(match seat_in_year(c, student_id, school_year)? {
        None => Some("지난 학년도".into()),
        Some((status, in_date, out_date)) => {
            // 아직 오지 않은 이동은 '예정' 으로 알린다 — 지금은 함께 다니는지
            // 아닌지가 오늘 기준으로 갈리기 때문이다
            match enroll::pending_on(&status, in_date.as_deref(), out_date.as_deref(), asof) {
                Some(p) => Some(p.label().to_string()),
                None if status == "TRANSFER_OUT" => Some("전출".into()),
                None => None,
            }
        }
    })
}

pub fn brief(
    c: &Connection,
    student_id: i64,
    school_year: i32,
    asof: NaiveDate,
) -> AppResult<Option<SiblingBrief>> {
    // **지금 함께 다니는** 형제만 센다. 전출한 형제는 관계가 남아 있어도
    // '본교 형제' 수에는 들지 않는다 — 명단의 숫자는 현재를 말해야 한다.
    let mut partners = Vec::new();
    for p in confirmed_partners(c, student_id)? {
        if together_now(c, p, school_year, asof)? {
            partners.push(p);
        }
    }
    if partners.is_empty() {
        return Ok(None);
    }
    let mut items = Vec::with_capacity(partners.len());
    for p in partners {
        items.push(labeled(c, p, school_year)?);
    }
    items.sort_by(|a, b| a.order.cmp(&b.order));
    let labels: Vec<String> = items.into_iter().map(|l| l.text).collect();
    Ok(Some(SiblingBrief {
        count: labels.len() as i64,
        text: labels.join(BRIEF_SEP),
        labels,
    }))
}

// ---------------------------------------------------------------
// 사용자 결정
// ---------------------------------------------------------------

fn set_status(c: &Connection, link_id: i64, status: &str) -> AppResult<(i64, i64)> {
    let pair: Option<(i64, i64)> = c
        .query_row(
            "SELECT student_a, student_b FROM sibling_links WHERE id = ?1",
            [link_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let pair = pair.ok_or_else(|| AppError::not_found("형제 관계를 찾을 수 없습니다."))?;

    c.execute(
        "UPDATE sibling_links
            SET status = ?2, decided_at = datetime('now','localtime')
          WHERE id = ?1",
        params![link_id, status],
    )?;
    Ok(pair)
}

/// 형제로 확인한다.
pub fn confirm(c: &Connection, link_id: i64) -> AppResult<(i64, i64)> {
    set_status(c, link_id, "CONFIRMED")
}

/// 형제가 아니라고 정한다. **다시 훑어도 후보로 되살아나지 않는다.**
pub fn reject(c: &Connection, link_id: i64) -> AppResult<(i64, i64)> {
    set_status(c, link_id, "REJECTED")
}

/// 결정을 도로 물린다. 다시 후보로 두고 사람이 새로 판단한다.
pub fn reset(c: &Connection, link_id: i64) -> AppResult<(i64, i64)> {
    let pair: Option<(i64, i64)> = c
        .query_row(
            "SELECT student_a, student_b FROM sibling_links WHERE id = ?1",
            [link_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let pair = pair.ok_or_else(|| AppError::not_found("형제 관계를 찾을 수 없습니다."))?;

    c.execute(
        "UPDATE sibling_links SET status = 'CANDIDATE', decided_at = NULL WHERE id = ?1",
        [link_id],
    )?;
    Ok(pair)
}

/// 형제에게 있는 보호자 정보를 **빈칸에만** 옮겨 담는다.
///
/// 사용자가 고른 항목만 채운다. 값이 있는 항목은 어떤 경우에도 덮어쓰지 않는다.
pub fn fill_from_sibling(
    c: &Connection,
    link_id: i64,
    target: i64,
    fields: &[Field],
) -> AppResult<Vec<Field>> {
    let link = c
        .query_row(
            "SELECT id, student_a, student_b, status FROM sibling_links WHERE id = ?1",
            [link_id],
            |r| {
                Ok(Link {
                    id: r.get(0)?,
                    student_a: r.get(1)?,
                    student_b: r.get(2)?,
                    status: r.get(3)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("형제 관계를 찾을 수 없습니다."))?;

    if link.student_a != target && link.student_b != target {
        return Err(AppError::invalid("이 형제 관계에 없는 학생입니다."));
    }
    if link.status != "CONFIRMED" {
        return Err(AppError::invalid(
            "형제로 확인한 뒤에 보호자 정보를 가져올 수 있습니다.",
        ));
    }

    let source = link.other(target);
    let mine = guardians_of(c, target)?;
    let theirs = guardians_of(c, source)?;

    // 실제로 채울 수 있는 것만 남긴다 — 사이에 값이 생겼을 수도 있다
    let allowed = sibling::fillable(&mine, &theirs);
    let to_fill: Vec<Field> = fields
        .iter()
        .copied()
        .filter(|f| allowed.contains(f))
        .collect();

    for f in &to_fill {
        let value = theirs.raw(*f).unwrap_or_default();
        match f {
            Field::FatherName => {
                c.execute(
                    "UPDATE students SET father_name = ?2,
                            updated_at = datetime('now','localtime')
                      WHERE id = ?1 AND (father_name IS NULL OR TRIM(father_name) = '')",
                    params![target, value],
                )?;
            }
            Field::MotherName => {
                c.execute(
                    "UPDATE students SET mother_name = ?2,
                            updated_at = datetime('now','localtime')
                      WHERE id = ?1 AND (mother_name IS NULL OR TRIM(mother_name) = '')",
                    params![target, value],
                )?;
            }
            Field::FatherPhone => {
                let (display, digits) = crate::domain::phone::normalize(value);
                c.execute(
                    "UPDATE students SET father_phone = ?2, father_phone_digits = ?3,
                            updated_at = datetime('now','localtime')
                      WHERE id = ?1 AND (father_phone IS NULL OR TRIM(father_phone) = '')",
                    params![target, display, digits],
                )?;
            }
            Field::MotherPhone => {
                let (display, digits) = crate::domain::phone::normalize(value);
                c.execute(
                    "UPDATE students SET mother_phone = ?2, mother_phone_digits = ?3,
                            updated_at = datetime('now','localtime')
                      WHERE id = ?1 AND (mother_phone IS NULL OR TRIM(mother_phone) = '')",
                    params![target, display, digits],
                )?;
            }
        }
    }
    Ok(to_fill)
}

// ---------------------------------------------------------------
// 전체 다시 훑기
// ---------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub scanned: i64,
    /// 이번에 새로 찾은 후보
    pub new_candidates: i64,
    /// 이미 있던 후보 (그대로 둔다)
    pub existing_candidates: i64,
    /// 사용자가 형제로 확인해 둔 관계
    pub confirmed: i64,
    /// 사용자가 형제가 아니라고 해 둔 관계
    pub rejected: i64,
    /// 근거가 사라져 지운 후보
    pub dropped: i64,
    /// 확정 형제에게서 가져올 수 있는 보호자 정보 건수
    pub guardian_fill: i64,
    /// 확정 형제와 값이 다른 건수
    pub guardian_conflict: i64,
    /// 너무 흔해서 쓰지 않은 값의 수
    pub skipped_values: i64,
}

fn save_fields(c: &Connection, link_id: i64, cmp: &Comparison) -> AppResult<()> {
    c.execute(
        "UPDATE sibling_links SET matched_fields = ?2, conflict_fields = ?3 WHERE id = ?1",
        params![
            link_id,
            sibling::to_json(&cmp.matched),
            sibling::to_json(&cmp.conflicts)
        ],
    )?;
    Ok(())
}

/// 그 학년도 재학생에서 형제 후보를 찾는다.
///
/// 사용자가 이미 정한 것(CONFIRMED·REJECTED)은 **상태를 건드리지 않고** 일치·불일치
/// 내용만 새로 고친다. 보호자 정보가 바뀌었을 수 있기 때문이다.
pub fn scan(
    c: &Connection,
    school_year: i32,
    asof: NaiveDate,
    mut on_progress: impl FnMut(&str, usize, usize),
) -> AppResult<ScanResult> {
    let entries = active_entries(c, school_year, asof)?;
    let mut out = ScanResult {
        scanned: entries.len() as i64,
        ..Default::default()
    };

    on_progress("PAIR", 0, entries.len());
    let found = sibling::find_pairs(&entries);
    out.skipped_values = found.skipped_values as i64;
    on_progress("PAIR", entries.len(), entries.len());

    let by_id: std::collections::HashMap<i64, &Guardians> = entries
        .iter()
        .map(|e| (e.student_id, &e.guardians))
        .collect();

    // 근거가 사라진 후보를 가려내려고, 이번에 후보로 남은 관계를 적어 둔다
    let mut still_candidate: std::collections::HashSet<i64> = std::collections::HashSet::new();

    let total = found.pairs.len();
    let step = (total / 100).max(1);
    for (i, (a, b)) in found.pairs.iter().enumerate() {
        let (Some(ga), Some(gb)) = (by_id.get(a), by_id.get(b)) else {
            continue;
        };
        let cmp = sibling::compare(ga, gb);
        if !cmp.is_candidate() {
            continue;
        }

        match find_link(c, *a, *b)? {
            Some(link) => {
                // 사용자가 정한 것은 그대로 두고 내용만 새로 고친다
                save_fields(c, link.id, &cmp)?;
                match link.status.as_str() {
                    "CANDIDATE" => {
                        out.existing_candidates += 1;
                        still_candidate.insert(link.id);
                    }
                    "CONFIRMED" => out.confirmed += 1,
                    _ => out.rejected += 1,
                }
            }
            None => {
                c.execute(
                    "INSERT INTO sibling_links(
                        student_a, student_b, status, source, matched_fields, conflict_fields)
                     VALUES (?1,?2,'CANDIDATE','AUTO',?3,?4)",
                    params![
                        a,
                        b,
                        sibling::to_json(&cmp.matched),
                        sibling::to_json(&cmp.conflicts)
                    ],
                )?;
                out.new_candidates += 1;
                still_candidate.insert(c.last_insert_rowid());
            }
        }

        if i % step == 0 {
            on_progress("COMPARE", i, total);
        }
    }
    on_progress("COMPARE", total, total);

    // 근거가 사라진 **후보**만 지운다. 사용자가 정한 것은 남긴다.
    let mut st = c.prepare("SELECT id FROM sibling_links WHERE status = 'CANDIDATE'")?;
    let all_candidates: Vec<i64> = st
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    drop(st);
    for id in all_candidates {
        if !still_candidate.contains(&id) {
            c.execute("DELETE FROM sibling_links WHERE id = ?1", [id])?;
            out.dropped += 1;
        }
    }

    // 확정 형제의 보호자 정보 상태를 센다
    on_progress("GUARDIAN", 0, 1);
    let mut st = c.prepare("SELECT id, student_a, student_b FROM sibling_links WHERE status = 'CONFIRMED'")?;
    let confirmed: Vec<(i64, i64, i64)> = st
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    drop(st);
    for (_, a, b) in confirmed {
        let ga = guardians_of(c, a)?;
        let gb = guardians_of(c, b)?;
        out.guardian_fill += sibling::fillable(&ga, &gb).len() as i64;
        out.guardian_fill += sibling::fillable(&gb, &ga).len() as i64;
        out.guardian_conflict += sibling::compare(&ga, &gb).conflicts.len() as i64;
    }
    on_progress("GUARDIAN", 1, 1);

    Ok(out)
}

/// 학생 **한 명**에 걸린 형제 후보만 찾는다. 새로 생긴 후보 수를 돌려준다.
///
/// 전입생 한 명 때문에 학교 전체를 다시 훑을 까닭은 없다. 버킷을 만드는 값도,
/// 견주는 규칙도 전체 훑기와 똑같고 **쓰는 범위만** 이 학생이 낀 쌍으로 좁힌다.
/// 그래서 결과가 전체 훑기와 어긋나지 않는다.
///
/// 사용자가 이미 정한 관계(CONFIRMED·REJECTED)는 여기서도 건드리지 않는다.
pub fn scan_for_student(
    c: &Connection,
    school_year: i32,
    student_id: i64,
    today: NaiveDate,
) -> AppResult<i64> {
    let entries = active_entries(c, school_year, today)?;
    if !entries.iter().any(|e| e.student_id == student_id) {
        return Ok(0); // 지금 다니지 않는 학생이면 찾을 것이 없다
    }
    let found = sibling::find_pairs(&entries);
    let by_id: std::collections::HashMap<i64, &Guardians> = entries
        .iter()
        .map(|e| (e.student_id, &e.guardians))
        .collect();

    let mut new_candidates = 0;
    let mut touched: Vec<i64> = vec![student_id];

    for (a, b) in found.pairs.iter().filter(|(a, b)| *a == student_id || *b == student_id) {
        let (Some(ga), Some(gb)) = (by_id.get(a), by_id.get(b)) else {
            continue;
        };
        let cmp = sibling::compare(ga, gb);
        if !cmp.is_candidate() {
            continue;
        }
        match find_link(c, *a, *b)? {
            Some(link) => save_fields(c, link.id, &cmp)?,
            None => {
                c.execute(
                    "INSERT INTO sibling_links(
                        student_a, student_b, status, source, matched_fields, conflict_fields)
                     VALUES (?1,?2,'CANDIDATE','AUTO',?3,?4)",
                    params![
                        a,
                        b,
                        sibling::to_json(&cmp.matched),
                        sibling::to_json(&cmp.conflicts)
                    ],
                )?;
                new_candidates += 1;
            }
        }
        touched.push(if *a == student_id { *b } else { *a });
    }

    // 걸린 학생들만 표시를 다시 맞춘다
    for id in touched {
        crate::repo::student::sync_issues(c, id, school_year, today)?;
    }
    Ok(new_candidates)
}

/// 지금 함께 다니는 확정 형제의 이름표.
///
/// 내보내기가 쓴다. 전출했거나 지난 학년도 형제는 넣지 않는다 — 명단에 적힌
/// '본교 형제' 는 지금 같이 다니는 학생을 뜻해야 한다.
pub fn labels_of(
    c: &Connection,
    student_id: i64,
    school_year: i32,
    asof: NaiveDate,
) -> AppResult<Vec<String>> {
    let mut out = Vec::new();
    for p in confirmed_partners(c, student_id)? {
        if together_now(c, p, school_year, asof)? {
            out.push(student_label_of(c, p, school_year)?);
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------
// 후보 한꺼번에 확정하기
// ---------------------------------------------------------------

/// 확인 필요 화면에 떠 있는 형제 후보 **한 쌍**.
///
/// 한 관계는 학생 둘에게 각각 표시가 뜬다. 여기서는 쌍마다 한 줄만 준다 —
/// 같은 관계를 두 번 확정할 까닭이 없기 때문이다.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CandidateRow {
    pub link_id: i64,
    pub student_a: i64,
    /// `3-나리 홍길동` — 지금 학적으로 그때그때 만든다
    pub label_a: String,
    pub student_b: i64,
    pub label_b: String,
    /// 후보로 본 근거 — `부 성명`, `모 연락처` 처럼
    pub matched: Vec<String>,
    /// 양쪽 다 값이 있는데 서로 다른 항목. 확정 전에 한 번 더 보라는 뜻이다.
    pub conflicts: Vec<String>,
    pub found_at: String,
}

/// 그 학년도 확인 필요 목록에 떠 있는 형제 후보.
///
/// 목록 화면과 **같은 것**을 보여야 하므로, 열려 있는 `SIBLING_CANDIDATE` 표시가
/// 걸린 관계만 고른다. 전출·졸업해서 표시가 닫힌 관계는 여기에도 나오지 않는다.
pub fn candidates(
    c: &Connection,
    school_year: i32,
    asof: NaiveDate,
) -> AppResult<Vec<CandidateRow>> {
    let active = enroll::active_sql("e2.", asof);
    let sql = format!(
        "SELECT l.id, l.student_a, l.student_b, l.matched_fields, l.conflict_fields, l.created_at
           FROM sibling_links l
           JOIN students s      ON s.id = l.student_a
           LEFT JOIN enrollments e ON e.student_id = l.student_a AND e.school_year = ?1
          WHERE l.status = 'CANDIDATE'
            AND EXISTS (
                SELECT 1 FROM issues i
                  JOIN enrollments e2 ON e2.student_id = i.student_id
                                     AND e2.school_year = ?1
                 WHERE i.ref_id = l.id
                   AND i.kind = 'SIBLING_CANDIDATE'
                   AND i.status = 'OPEN'
                   AND {active})
          ORDER BY {order}, l.id",
        order = label::ORDER_BY_ROSTER,
    );

    let mut st = c.prepare(&sql)?;
    let raw: Vec<(i64, i64, i64, Option<String>, Option<String>, String)> = st
        .query_map([school_year], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        })?
        .collect::<rusqlite::Result<_>>()?;
    drop(st);

    let names = |json: Option<String>| -> Vec<String> {
        sibling::from_json(json.as_deref().unwrap_or("[]"))
            .iter()
            .map(|f| f.label().to_string())
            .collect()
    };

    let mut out = Vec::with_capacity(raw.len());
    for (link_id, a, b, matched, conflicts, found_at) in raw {
        out.push(CandidateRow {
            link_id,
            label_a: student_label_of(c, a, school_year)?,
            label_b: student_label_of(c, b, school_year)?,
            student_a: a,
            student_b: b,
            matched: names(matched),
            conflicts: names(conflicts),
            found_at,
        });
    }
    Ok(out)
}

/// 후보 여럿을 한꺼번에 확정한 결과.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchConfirm {
    /// 이번에 후보에서 형제로 바뀐 수
    pub confirmed: i64,
    /// 이미 형제로 확정되어 있던 수 (다시 세지 않는다)
    pub already: i64,
    /// '형제 아님' 으로 정해 둔 것이라 건드리지 않은 수
    pub skipped: i64,
    /// 표시를 다시 맞춰야 하는 학생
    pub students: Vec<i64>,
}

/// 고른 후보를 형제로 확정한다.
///
/// 지키는 것
///   * **모두 되거나 모두 안 된다.** 없는 관계 번호가 하나라도 있으면 오류를 내고,
///     부르는 쪽(`Db::write`)의 트랜잭션이 되돌려 놓는다. 절반만 확정된 상태로
///     끝나지 않는다.
///   * 사용자가 '형제 아님'(REJECTED)이라고 해 둔 관계는 **일괄 확정으로도 뒤집지
///     않는다.** 건너뛰고 몇 건인지만 알린다.
///   * 이미 확정된 관계는 오류가 아니다. 목록을 띄워 둔 사이에 딴 데서 확정했을 수 있다.
pub fn confirm_many(c: &Connection, link_ids: &[i64]) -> AppResult<BatchConfirm> {
    let mut out = BatchConfirm::default();
    let mut seen: std::collections::HashSet<i64> = std::collections::HashSet::new();
    let mut students: std::collections::HashSet<i64> = std::collections::HashSet::new();

    for id in link_ids {
        if !seen.insert(*id) {
            continue; // 같은 관계를 두 번 보내도 한 번만 센다
        }
        let row: Option<(i64, i64, String)> = c
            .query_row(
                "SELECT student_a, student_b, status FROM sibling_links WHERE id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        let Some((a, b, status)) = row else {
            return Err(AppError::not_found(
                "형제 후보 가운데 사라진 것이 있어 아무것도 확정하지 않았습니다. \
                 목록을 새로 고친 뒤 다시 해 주세요.",
            ));
        };

        match status.as_str() {
            "CANDIDATE" => {
                confirm(c, *id)?;
                out.confirmed += 1;
                students.insert(a);
                students.insert(b);
            }
            "CONFIRMED" => {
                out.already += 1;
                students.insert(a);
                students.insert(b);
            }
            // REJECTED — 사용자가 정한 것을 일괄 처리가 뒤집지 않는다
            _ => out.skipped += 1,
        }
    }

    let mut students: Vec<i64> = students.into_iter().collect();
    students.sort_unstable();
    out.students = students;
    Ok(out)
}

#[cfg(test)]
#[path = "sibling_tests.rs"]
mod sibling_tests;
