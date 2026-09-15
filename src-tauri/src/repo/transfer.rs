//! 전입·전출 — 학적 상태와 이동 이력.
//!
//! **학생을 새로 복제하거나 지우는 기능이 아니다.** 사람은 하나로 두고 학적의
//! 상태만 바꾸며, 언제 무슨 일이 있었는지를 `enrollment_events` 에 덧붙인다.
//!
//! 지키는 것
//!   * 한 학년도에 학적은 하나다. 같은 해에 나갔다 돌아와도 **행을 더 만들지 않는다.**
//!     되살리고, 나간 기록은 사건으로 남긴다.
//!   * 전출해도 학생과 학적을 지우지 않는다. 상태만 `TRANSFER_OUT` 이 된다.
//!   * 학생정보 정규화·주소 판정·확인 필요는 `repo::student` 것을 그대로 쓴다.
//!     전입 전용 규칙을 따로 만들지 않는다.
//!   * 같은 이동을 두 번 눌러도 사건이 두 번 쌓이지 않는다.

use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::domain::enroll::{self, DateProblem};
use crate::domain::label;
use crate::error::{AppError, AppResult};
use crate::repo::sibling as sibling_repo;
use crate::repo::student::{self, Entry, StudentInput};

// ---------------------------------------------------------------
// 기존 학생 찾기
// ---------------------------------------------------------------

/// 찾은 학생 하나. 사용자가 **같은 사람인지 보고 정할** 수 있도록 지난 학적을 함께 준다.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudentMatch {
    pub student_id: i64,
    pub name: String,
    pub gender: Option<String>,
    pub birth_date: Option<String>,
    pub birth_raw: Option<String>,
    /// 학년도별 학적 한 줄씩 (최근 학년도부터)
    pub history: Vec<HistoryLine>,
    /// 지금 학년도에 이미 학적이 있는지 — 있으면 상태 이름
    pub current_status: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryLine {
    pub school_year: i32,
    /// `2-나리 7번`
    pub where_at: String,
    pub status_label: String,
    /// `2026.05.14. 전출` 처럼 덧붙일 말
    pub extra: Option<String>,
}

/// 이름·생년월일로 기존 학생을 찾는다.
///
/// **프로그램은 같은 학생이라고 정하지 않는다.** 이름이 같다는 것만으로는 알 수 없다.
/// 후보를 지난 학적과 함께 보여 주고, 고르는 것은 사람이 한다.
pub fn search_students(
    c: &Connection,
    name: &str,
    birth: Option<&str>,
    school_year: i32,
) -> AppResult<Vec<StudentMatch>> {
    let name = name.trim();
    if name.chars().count() < 2 {
        return Err(AppError::invalid("찾을 이름을 두 글자 이상 적어 주세요."));
    }
    let birth = birth.map(str::trim).filter(|s| !s.is_empty());

    let mut st = c.prepare(
        "SELECT id, name, gender, birth_date, birth_raw
           FROM students
          WHERE name LIKE ?1
            AND (?2 IS NULL OR birth_date = ?2)
          ORDER BY name, birth_date
          LIMIT 30",
    )?;
    let found: Vec<(i64, String, Option<String>, Option<String>, Option<String>)> = st
        .query_map(params![format!("%{name}%"), birth], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })?
        .collect::<rusqlite::Result<_>>()?;
    drop(st);

    let mut out = Vec::with_capacity(found.len());
    for (id, name, gender, birth_date, birth_raw) in found {
        let history = history_of(c, id)?;
        let current_status = history
            .iter()
            .find(|h| h.school_year == school_year)
            .map(|h| h.status_label.clone());
        out.push(StudentMatch {
            student_id: id,
            name,
            gender,
            birth_date,
            birth_raw,
            history,
            current_status,
        });
    }
    Ok(out)
}

fn history_of(c: &Connection, student_id: i64) -> AppResult<Vec<HistoryLine>> {
    let mut st = c.prepare(
        "SELECT school_year, grade, class_name, class_no, status,
                transfer_in_date, transfer_out_date
           FROM enrollments
          WHERE student_id = ?1
          ORDER BY school_year DESC",
    )?;
    let rows = st
        .query_map([student_id], |r| {
            let grade: i32 = r.get(1)?;
            let class_name: Option<String> = r.get(2)?;
            let class_no: Option<i32> = r.get(3)?;
            let status: String = r.get(4)?;
            let in_date: Option<String> = r.get(5)?;
            let out_date: Option<String> = r.get(6)?;

            let mut where_at = label::class_label(grade, class_name.as_deref());
            if let Some(n) = class_no {
                where_at.push_str(&format!(" {n}번"));
            }
            let status_label = enroll::Status::parse(&status)
                .map(|s| s.label())
                .unwrap_or("재학")
                .to_string();

            let extra = match status.as_str() {
                "TRANSFER_OUT" => out_date.map(|d| format!("{} 전출", dot(&d))),
                "TRANSFER_IN" => in_date.map(|d| format!("{} 전입", dot(&d))),
                _ => None,
            };
            Ok(HistoryLine {
                school_year: r.get(0)?,
                where_at,
                status_label,
                extra,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// `2026-05-14` → `2026.05.14.`
fn dot(iso: &str) -> String {
    enroll::parse_date(iso).map(enroll::format_date).unwrap_or_else(|| iso.to_string())
}

// ---------------------------------------------------------------
// 사건 남기기 (중복 안전)
// ---------------------------------------------------------------

/// 그 학년도의 가장 마지막 사건 (종류, 날짜).
fn last_event(c: &Connection, student_id: i64, school_year: i32) -> AppResult<Option<(String, Option<String>)>> {
    let row = c
        .query_row(
            "SELECT kind, event_date FROM enrollment_events
              WHERE student_id = ?1 AND school_year = ?2
              ORDER BY id DESC LIMIT 1",
            params![student_id, school_year],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    Ok(row)
}

/// 같은 이동을 두 번 적지 않는다.
///
/// 버튼을 두 번 누르거나 명령이 두 번 도착해도 **마지막 사건과 종류·날짜가 같으면**
/// 새로 적지 않는다. 취소한 뒤에 같은 날로 다시 처리하는 경우는 마지막 사건이
/// `CANCEL` 이므로 정상적으로 적힌다.
#[allow(clippy::too_many_arguments)]
fn add_event_once(
    c: &Connection,
    student_id: i64,
    school_year: i32,
    kind: &str,
    date: Option<&str>,
    grade: i32,
    class_name: Option<&str>,
    class_no: Option<i32>,
    note: Option<&str>,
) -> AppResult<bool> {
    if let Some((last_kind, last_date)) = last_event(c, student_id, school_year)? {
        if last_kind == kind && last_date.as_deref() == date {
            return Ok(false);
        }
    }
    student::add_event(
        c,
        student_id,
        school_year,
        kind,
        date,
        grade,
        class_name,
        class_no,
        note,
        "MANUAL",
    )?;
    Ok(true)
}

/// 이동 날짜를 따진다. 앞선 사건보다 이른 날짜는 이력을 뒤집으므로 막는다.
fn check(
    c: &Connection,
    student_id: Option<i64>,
    school_year: i32,
    raw: &str,
    what: &str,
    today: NaiveDate,
) -> AppResult<String> {
    let last = match student_id {
        Some(id) => last_event(c, id, school_year)?
            .and_then(|(kind, date)| {
                date.and_then(|d| enroll::parse_date(&d))
                    .map(|d| (student::event_label(&kind), d))
            }),
        None => None,
    };
    match enroll::check_date(raw, school_year, today, last) {
        Ok(d) => Ok(d.to_string()),
        Err(e) => Err(AppError::invalid(problem_message(&e, what, school_year))),
    }
}

fn problem_message(e: &DateProblem, what: &str, school_year: i32) -> String {
    e.message(what, school_year)
}

// ---------------------------------------------------------------
// 전입
// ---------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferInInput {
    /// 이미 있는 학생이면 그 번호. 새 학생이면 None
    pub student_id: Option<i64>,
    /// 기본정보와 학적 — 학생 등록 폼 그대로다
    pub student: StudentInput,
    /// `YYYY-MM-DD`
    pub date: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferInResult {
    pub student_id: i64,
    /// 학생을 새로 만들었는가
    pub created: bool,
    /// 같은 학년도 학적을 되살렸는가 (나갔다 돌아온 경우)
    pub returned: bool,
    /// 이번에 새로 찾은 본교 형제 후보
    pub sibling_candidates: i64,
    /// 같은 반에 같은 번호를 쓰는 학생이 생겼는가
    pub number_dup: bool,
    pub student_label: String,
}

/// 전입을 처리한다.
pub fn transfer_in(
    c: &Connection,
    input: &TransferInInput,
    today: NaiveDate,
) -> AppResult<TransferInResult> {
    let year = input.student.school_year;
    let date = check(c, input.student_id, year, &input.date, "전입일", today)?;
    let class_name = input
        .student
        .class_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let mut out = TransferInResult::default();

    let student_id = match input.student_id {
        None => {
            out.created = true;
            student::create_with(
                c,
                &input.student,
                today,
                &Entry {
                    status: "TRANSFER_IN",
                    kind: "TRANSFER_IN",
                    date: Some(&date),
                    note: None,
                    source: "MANUAL",
                },
            )?
        }
        Some(id) => {
            // 그 학년도 학적이 있으면 되살리고, 없으면 새로 만든다.
            // 어느 쪽이든 학생 행은 하나뿐이다.
            let has = student::ensure_enrollment(
                c,
                id,
                year,
                input.student.grade,
                class_name,
                input.student.class_no,
                "MANUAL",
            )?;
            out.returned = !has; // ensure 가 만들지 않았다 = 이미 있었다 = 돌아온 것
            // 기본정보와 학적을 한 번에 갱신한다 (주소·확인 필요도 여기서 다시 따진다)
            student::update(c, id, &input.student, today)?;
            id
        }
    };

    // 학적 상태를 전입으로 맞춘다.
    //
    // 나간 기록은 `enrollment_events` 에 남아 있으므로 학적의 전출 칸은 비운다 —
    // 이 칸은 '가장 최근 값의 사본' 이고, 지금 가장 최근 일은 돌아온 것이다.
    c.execute(
        "UPDATE enrollments
            SET status = 'TRANSFER_IN',
                transfer_in_date = ?3,
                transfer_out_date = NULL,
                transfer_out_to = NULL,
                transfer_out_note = NULL,
                updated_at = datetime('now','localtime')
          WHERE student_id = ?1 AND school_year = ?2",
        params![student_id, year, date],
    )?;

    // 새 학생은 create_with 가 이미 사건을 남겼다
    if !out.created {
        add_event_once(
            c,
            student_id,
            year,
            "TRANSFER_IN",
            Some(&date),
            input.student.grade,
            class_name,
            input.student.class_no,
            None,
        )?;
    }

    // 주소·번호 중복·형제까지 표시를 맞춘다
    student::sync_issues(c, student_id, year, today)?;
    student::sync_number_peers(
        c,
        year,
        input.student.grade,
        class_name,
        &input.student.class_no.into_iter().collect::<Vec<i32>>(),
        student_id,
        today,
    )?;

    // 새로 들어온 학생 한 명에 걸린 형제만 찾는다. 학교 전체를 다시 훑지 않는다.
    let scan = sibling_repo::scan_for_student(c, year, student_id, today)?;
    out.sibling_candidates = scan;

    out.number_dup = c.query_row(
        "SELECT COUNT(*) FROM issues
          WHERE student_id = ?1 AND kind = 'NUMBER_DUP' AND status = 'OPEN'",
        [student_id],
        |r| r.get::<_, i64>(0),
    )? > 0;
    out.student_id = student_id;
    out.student_label = sibling_repo::student_label_of(c, student_id, year)?;
    Ok(out)
}

// ---------------------------------------------------------------
// 전출
// ---------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferOutInput {
    pub student_id: i64,
    pub school_year: i32,
    /// `YYYY-MM-DD`
    pub date: String,
    /// 어디로 갔는지 — 아는 만큼만
    pub to_school: Option<String>,
    pub note: Option<String>,
}

/// 전출 처리. **학생도 학적도 지우지 않는다.**
pub fn transfer_out(c: &Connection, input: &TransferOutInput, today: NaiveDate) -> AppResult<()> {
    let year = input.school_year;
    let date = check(c, Some(input.student_id), year, &input.date, "전출일", today)?;

    let seat: Option<(i32, Option<String>, Option<i32>, String)> = c
        .query_row(
            "SELECT grade, class_name, class_no, status FROM enrollments
              WHERE student_id = ?1 AND school_year = ?2",
            params![input.student_id, year],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    let (grade, class_name, class_no, status) = seat.ok_or_else(|| {
        AppError::not_found(format!("이 학생에게는 {year}학년도 학적이 없습니다."))
    })?;
    if status == "TRANSFER_OUT" {
        return Err(AppError::invalid("이미 전출 처리된 학생입니다."));
    }

    let to_school = clean(&input.to_school);
    let note = clean(&input.note);

    c.execute(
        "UPDATE enrollments
            SET status = 'TRANSFER_OUT',
                transfer_out_date = ?3,
                transfer_out_to = ?4,
                transfer_out_note = ?5,
                updated_at = datetime('now','localtime')
          WHERE student_id = ?1 AND school_year = ?2",
        params![input.student_id, year, date, to_school, note],
    )?;

    // 그때의 학년·반·번호를 사건에 함께 남긴다. 나중에 반이 바뀌어도 기록이 흐트러지지 않는다.
    add_event_once(
        c,
        input.student_id,
        year,
        "TRANSFER_OUT",
        Some(&date),
        grade,
        class_name.as_deref(),
        class_no,
        event_note(to_school.as_deref(), note.as_deref()).as_deref(),
    )?;

    after_move(c, input.student_id, year, grade, class_name.as_deref(), class_no, today)
}

/// 전출 처리를 되돌린다.
///
/// 사건을 지우지 않는다. **무슨 일이 있었는지는 남기고 상태만 되돌린다** — 지워 버리면
/// 잘못 눌렀다는 사실조차 사라져 나중에 무엇이 맞는지 알 수 없다.
pub fn transfer_out_cancel(
    c: &Connection,
    student_id: i64,
    school_year: i32,
    today: NaiveDate,
) -> AppResult<()> {
    let seat: Option<(i32, Option<String>, Option<i32>, String)> = c
        .query_row(
            "SELECT grade, class_name, class_no, status FROM enrollments
              WHERE student_id = ?1 AND school_year = ?2",
            params![student_id, school_year],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    let (grade, class_name, class_no, status) = seat.ok_or_else(|| {
        AppError::not_found(format!("이 학생에게는 {school_year}학년도 학적이 없습니다."))
    })?;
    if status != "TRANSFER_OUT" {
        return Err(AppError::invalid("전출 처리된 학생이 아닙니다."));
    }

    // 나가기 전이 전입생이었으면 전입생으로 되돌린다
    let was_transfer_in: i64 = c.query_row(
        "SELECT COUNT(*) FROM enrollment_events
          WHERE student_id = ?1 AND school_year = ?2 AND kind = 'TRANSFER_IN'",
        params![student_id, school_year],
        |r| r.get(0),
    )?;
    let back = if was_transfer_in > 0 {
        "TRANSFER_IN"
    } else {
        "ENROLLED"
    };

    c.execute(
        "UPDATE enrollments
            SET status = ?3,
                transfer_out_date = NULL,
                transfer_out_to = NULL,
                transfer_out_note = NULL,
                updated_at = datetime('now','localtime')
          WHERE student_id = ?1 AND school_year = ?2",
        params![student_id, school_year, back],
    )?;

    student::add_event(
        c,
        student_id,
        school_year,
        "CANCEL",
        None,
        grade,
        class_name.as_deref(),
        class_no,
        Some("전출 처리 취소"),
        "MANUAL",
    )?;

    after_move(
        c,
        student_id,
        school_year,
        grade,
        class_name.as_deref(),
        class_no,
        today,
    )
}

/// 상태가 바뀐 뒤 표시를 맞춘다.
///
/// 본인과 **같은 번호를 쓰던 같은 반 학생**까지 본다. 전출로 자리가 비면 남은 학생의
/// '번호 중복' 이 풀리고, 되돌아오면 다시 켜져야 하기 때문이다.
fn after_move(
    c: &Connection,
    student_id: i64,
    school_year: i32,
    grade: i32,
    class_name: Option<&str>,
    class_no: Option<i32>,
    today: NaiveDate,
) -> AppResult<()> {
    student::sync_issues(c, student_id, school_year, today)?;
    let numbers: Vec<i32> = class_no.into_iter().collect();
    student::sync_number_peers(c, school_year, grade, class_name, &numbers, student_id, today)?;
    // 형제 쪽 표시도 다시 맞춘다 (전출한 형제는 본교 형제 수에서 빠진다)
    for other in sibling_repo::confirmed_partners(c, student_id)? {
        student::sync_issues(c, other, school_year, today).ok();
    }
    Ok(())
}

fn event_note(to_school: Option<&str>, note: Option<&str>) -> Option<String> {
    let parts: Vec<&str> = [to_school, note].into_iter().flatten().collect();
    (!parts.is_empty()).then(|| parts.join(" · "))
}

fn clean(v: &Option<String>) -> Option<String> {
    v.as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

// ---------------------------------------------------------------
// 지난 전출생 직접 넣기
// ---------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PastOutInput {
    /// 이미 있는 학생이면 그 번호
    pub student_id: Option<i64>,
    pub student: StudentInput,
    pub date: String,
    pub to_school: Option<String>,
    pub note: Option<String>,
}

/// 프로그램을 쓰기 전에 이미 나간 학생을 뒤늦게 적어 넣는다.
///
/// 명단에 넣었다가 다시 전출 처리하는 번거로운 길을 거치지 않는다. 처음부터
/// 전출 상태로 만들고, 현재 학생명단에는 나타나지 않는다.
pub fn add_past_transfer_out(
    c: &Connection,
    input: &PastOutInput,
    today: NaiveDate,
) -> AppResult<i64> {
    let year = input.student.school_year;
    let date = check(c, input.student_id, year, &input.date, "전출일", today)?;
    let class_name = input
        .student
        .class_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let to_school = clean(&input.to_school);
    let note = clean(&input.note);

    let student_id = match input.student_id {
        None => student::create_with(
            c,
            &input.student,
            today,
            &Entry {
                status: "TRANSFER_OUT",
                kind: "TRANSFER_OUT",
                date: Some(&date),
                note: event_note(to_school.as_deref(), note.as_deref()).as_deref(),
                source: "MANUAL",
            },
        )?,
        Some(id) => {
            student::ensure_enrollment(
                c,
                id,
                year,
                input.student.grade,
                class_name,
                input.student.class_no,
                "MANUAL",
            )?;
            student::update(c, id, &input.student, today)?;
            add_event_once(
                c,
                id,
                year,
                "TRANSFER_OUT",
                Some(&date),
                input.student.grade,
                class_name,
                input.student.class_no,
                event_note(to_school.as_deref(), note.as_deref()).as_deref(),
            )?;
            id
        }
    };

    c.execute(
        "UPDATE enrollments
            SET status = 'TRANSFER_OUT',
                transfer_out_date = ?3,
                transfer_out_to = ?4,
                transfer_out_note = ?5,
                updated_at = datetime('now','localtime')
          WHERE student_id = ?1 AND school_year = ?2",
        params![student_id, year, date, to_school, note],
    )?;

    after_move(
        c,
        student_id,
        year,
        input.student.grade,
        class_name,
        input.student.class_no,
        today,
    )?;
    Ok(student_id)
}

// ---------------------------------------------------------------
// 목록
// ---------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveRow {
    pub student_id: i64,
    pub name: String,
    pub gender: Option<String>,
    pub grade: i32,
    pub class_name: Option<String>,
    pub class_no: Option<i32>,
    pub class_label: String,
    /// 전입생 화면이면 전입일, 전출생 화면이면 전출일
    pub date: Option<String>,
    pub to_school: Option<String>,
    pub note: Option<String>,
    pub status: String,
    pub status_label: String,
    pub issue_count: i64,
}

/// 그 학년도의 전입생 또는 전출생 목록.
///
/// `want_in` 이 true 면 전입생(지금 다니는 학생), false 면 전출생이다.
pub fn list(
    c: &Connection,
    school_year: i32,
    want_in: bool,
    grade: Option<i32>,
    q: Option<&str>,
) -> AppResult<Vec<MoveRow>> {
    let status = if want_in { "TRANSFER_IN" } else { "TRANSFER_OUT" };
    let date_col = if want_in {
        "e.transfer_in_date"
    } else {
        "e.transfer_out_date"
    };
    let q = q.map(str::trim).filter(|s| !s.is_empty());

    let sql = format!(
        "SELECT e.student_id, s.name, s.gender, e.grade, e.class_name, e.class_no,
                {date_col}, e.transfer_out_to, e.transfer_out_note, e.status,
                (SELECT COUNT(*) FROM issues i
                  WHERE i.student_id = e.student_id AND i.status = 'OPEN')
           FROM enrollments e
           JOIN students s ON s.id = e.student_id
          WHERE e.school_year = ?1 AND e.status = ?2
            AND (?3 IS NULL OR e.grade = ?3)
            AND (?4 IS NULL OR s.name LIKE ?4)
          ORDER BY {date_col} DESC, {order}",
        order = label::ORDER_BY_ROSTER,
    );
    let mut st = c.prepare(&sql)?;
    let rows = st
        .query_map(
            params![school_year, status, grade, q.map(|v| format!("%{v}%"))],
            |r| {
                let grade: i32 = r.get(3)?;
                let class_name: Option<String> = r.get(4)?;
                let status: String = r.get(9)?;
                Ok(MoveRow {
                    student_id: r.get(0)?,
                    name: r.get(1)?,
                    gender: r.get(2)?,
                    class_label: label::class_label(grade, class_name.as_deref()),
                    grade,
                    class_name,
                    class_no: r.get(5)?,
                    date: r.get(6)?,
                    to_school: r.get(7)?,
                    note: r.get(8)?,
                    status_label: enroll::Status::parse(&status)
                        .map(|s| s.label())
                        .unwrap_or("재학")
                        .to_string(),
                    status,
                    issue_count: r.get(10)?,
                })
            },
        )?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

#[cfg(test)]
#[path = "transfer_tests.rs"]
mod transfer_tests;
