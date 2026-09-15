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
use crate::domain::enroll::{self, ACTIVE_STATUS_SQL as ACTIVE};
use crate::error::{AppError, AppResult};

// ---------------------------------------------------------------
// 읽기
// ---------------------------------------------------------------

/// 그 학년도 재학생(재학·전입)의 보호자 정보.
///
/// 전출·졸업한 학생은 훑기에서 뺀다. 관계 자체는 학생 번호로 남으므로
/// 지난 기록은 그대로 볼 수 있다.
pub fn active_entries(c: &Connection, school_year: i32) -> AppResult<Vec<Entry>> {
    let mut st = c.prepare(&format!(
        "SELECT s.id, s.father_name, s.mother_name, s.father_phone, s.mother_phone
           FROM students s
           JOIN enrollments e ON e.student_id = s.id
          WHERE e.school_year = ?1 AND e.{ACTIVE}
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
pub fn student_label_of(c: &Connection, student_id: i64, school_year: i32) -> AppResult<String> {
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
        Some((name, grade, class_name)) => {
            label::student_label(grade, class_name.as_deref(), &name)
        }
        // 학적이 하나도 없는 학생 (있을 수 없지만 이름만이라도 보여 준다)
        None => c
            .query_row("SELECT name FROM students WHERE id = ?1", [student_id], |r| {
                r.get(0)
            })
            .optional()?
            .unwrap_or_else(|| "알 수 없는 학생".into()),
    })
}

/// 한 학생의 형제 관계를 화면에 보여 줄 모양으로.
pub fn list_for_student(
    c: &Connection,
    student_id: i64,
    school_year: i32,
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
            partner_note: partner_note(c, other, school_year)?,
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

/// 학생명단에 보여 줄 짧은 형제 표시.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SiblingBrief {
    pub count: i64,
    /// 한 명이면 `1-나리 홍길동`, 여럿이면 `2명`
    pub text: String,
}

/// 그 학년도에 이 학생이 어떤 상태인지. 지금 학적이 없으면 None.
///
/// 형제가 전출했거나 지난 학년도 학생이면 표시를 달리해야 한다 — 관계는 그대로
/// 두되 **지금 본교에 함께 다니는 것은 아니라는 사실**을 알려 주기 위해서다.
fn status_in_year(c: &Connection, student_id: i64, school_year: i32) -> AppResult<Option<String>> {
    Ok(c.query_row(
        "SELECT status FROM enrollments WHERE student_id = ?1 AND school_year = ?2",
        params![student_id, school_year],
        |r| r.get(0),
    )
    .optional()?)
}

/// 지금 본교에 함께 다니는 형제인가.
fn together_now(c: &Connection, student_id: i64, school_year: i32) -> AppResult<bool> {
    Ok(status_in_year(c, student_id, school_year)?
        .and_then(|s| enroll::Status::parse(&s))
        .map(|s| s.is_active())
        .unwrap_or(false))
}

/// 형제 이름표 뒤에 붙일 말. 함께 다니고 있으면 None.
/// 졸업을 먼저 본다 — 졸업생은 그 뒤 학년도에 학적이 없으므로 그냥 두면
/// '지난 학년도' 로 보인다. 왜 함께 다니지 않는지를 바로 말해야 한다.
fn partner_note(c: &Connection, student_id: i64, school_year: i32) -> AppResult<Option<String>> {
    let graduated: i64 = c.query_row(
        "SELECT COUNT(*) FROM graduations WHERE student_id = ?1 AND school_year <= ?2",
        params![student_id, school_year],
        |r| r.get(0),
    )?;
    if graduated > 0 {
        return Ok(Some("졸업".into()));
    }
    Ok(match status_in_year(c, student_id, school_year)? {
        Some(s) if s == "TRANSFER_OUT" => Some("전출".into()),
        Some(_) => None,
        None => Some("지난 학년도".into()),
    })
}

pub fn brief(c: &Connection, student_id: i64, school_year: i32) -> AppResult<Option<SiblingBrief>> {
    // **지금 함께 다니는** 형제만 센다. 전출한 형제는 관계가 남아 있어도
    // '본교 형제' 수에는 들지 않는다 — 명단의 숫자는 현재를 말해야 한다.
    let mut partners = Vec::new();
    for p in confirmed_partners(c, student_id)? {
        if together_now(c, p, school_year)? {
            partners.push(p);
        }
    }
    Ok(match partners.len() {
        0 => None,
        1 => Some(SiblingBrief {
            count: 1,
            text: student_label_of(c, partners[0], school_year)?,
        }),
        n => Some(SiblingBrief {
            count: n as i64,
            text: format!("{n}명"),
        }),
    })
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
    mut on_progress: impl FnMut(&str, usize, usize),
) -> AppResult<ScanResult> {
    let entries = active_entries(c, school_year)?;
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
    let entries = active_entries(c, school_year)?;
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
pub fn labels_of(c: &Connection, student_id: i64, school_year: i32) -> AppResult<Vec<String>> {
    let mut out = Vec::new();
    for p in confirmed_partners(c, student_id)? {
        if together_now(c, p, school_year)? {
            out.push(student_label_of(c, p, school_year)?);
        }
    }
    Ok(out)
}

#[cfg(test)]
#[path = "sibling_tests.rs"]
mod sibling_tests;
