//! 학생 기본정보 · 학년도별 학적 읽고 쓰기.
//!
//! 저장할 때 지키는 것
//!   * 원본(`birth_raw`, `address_raw`)은 손대지 않는다.
//!   * 정리값(`birth_date`, `*_phone_digits`, `address_norm`)은 `domain` 이 만든다.
//!   * 학적을 바꾸면 `enrollment_events` 에 사건을 남긴다 — 이력의 원본은 그쪽이다.
//!   * 확인할 것이 있어도 저장은 막지 않고 `issues` 에 표시만 남긴다.

use chrono::NaiveDate;
use rusqlite::{params, params_from_iter, types::Value, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::domain::enroll::ACTIVE_STATUS_SQL as ACTIVE;
use crate::domain::{address, birth, korean, label, phone, sibling};
use crate::repo::address as addr_repo;
use crate::repo::sibling as sibling_repo;
use crate::repo::stats;
use crate::error::{AppError, AppResult};
use crate::repo::issue::{self, IssueKind, IssueRow};

// ---------------------------------------------------------------
// 입력
// ---------------------------------------------------------------

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StudentInput {
    // 기본정보
    pub name: String,
    pub gender: Option<String>,
    pub birth_raw: Option<String>,
    pub address_raw: Option<String>,
    pub father_name: Option<String>,
    pub mother_name: Option<String>,
    pub father_phone: Option<String>,
    pub mother_phone: Option<String>,
    pub primary_phone: Option<String>,
    pub note: Option<String>,
    // 학적
    pub school_year: i32,
    pub grade: i32,
    pub class_name: Option<String>,
    pub class_no: Option<i32>,
    /// 주소를 바꿀 때 직접 지정한 분류(MANUAL)를 그대로 둘지.
    ///
    /// 기본은 false — 주소가 달라지면 예전 분류가 새 주소에도 맞다고 볼 수 없으므로
    /// 다시 판정한다. 사용자가 "그대로 두겠다" 고 고른 경우에만 true 로 온다.
    #[serde(default)]
    pub keep_manual_address: bool,
}

impl StudentInput {
    /// 저장 전 검사. 요구사항상 **학년도·학년·이름**만 필수다.
    pub fn validate(&self) -> AppResult<()> {
        if self.name.trim().is_empty() {
            return Err(AppError::invalid("학생 이름을 입력해 주세요."));
        }
        if self.name.chars().count() > 30 {
            return Err(AppError::invalid("이름은 30자 이내로 입력해 주세요."));
        }
        if !(1..=6).contains(&self.grade) {
            return Err(AppError::invalid("학년은 1~6 사이로 골라 주세요."));
        }
        if let Some(n) = self.class_no {
            if !(1..=200).contains(&n) {
                return Err(AppError::invalid("번호는 1~200 사이로 입력해 주세요."));
            }
        }
        if let Some(g) = self.gender.as_deref() {
            if !g.is_empty() && g != "M" && g != "F" {
                return Err(AppError::invalid("성별 값이 올바르지 않습니다."));
            }
        }
        Ok(())
    }
}

/// 빈 문자열은 None 으로 (화면에서 지운 값이 빈 문자열로 들어온다).
fn clean(v: &Option<String>) -> Option<String> {
    v.as_ref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}



// ---------------------------------------------------------------
// 출력
// ---------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudentRow {
    pub id: i64,
    pub name: String,
    pub gender: Option<String>,
    pub birth_date: Option<String>,
    pub birth_raw: Option<String>,
    pub grade: i32,
    pub class_name: Option<String>,
    pub class_no: Option<i32>,
    pub class_label: String,
    pub status: String,
    pub graduated: bool,
    pub address_raw: Option<String>,
    pub address_category: Option<String>,
    pub father_phone: Option<String>,
    pub mother_phone: Option<String>,
    pub primary_phone: Option<String>,
    pub issue_count: i64,
    /// 확정된 본교 형제. 없으면 None
    pub sibling: Option<sibling_repo::SiblingBrief>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudentDetail {
    pub id: i64,
    pub name: String,
    pub gender: Option<String>,
    pub birth_raw: Option<String>,
    pub birth_date: Option<String>,
    pub address_raw: Option<String>,
    pub address_category_id: Option<i64>,
    pub address_category: Option<String>,
    pub address_source: String,
    pub father_name: Option<String>,
    pub mother_name: Option<String>,
    pub father_phone: Option<String>,
    pub mother_phone: Option<String>,
    pub primary_phone: Option<String>,
    pub note: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    /// 지금 보고 있는 학년도의 학적
    pub enrollment: Option<EnrollmentRow>,
    /// 모든 학년도 학적 (최근 학년도부터)
    pub enrollments: Vec<EnrollmentRow>,
    pub events: Vec<EventRow>,
    pub issues: Vec<IssueRow>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnrollmentRow {
    pub school_year: i32,
    pub grade: i32,
    pub class_name: Option<String>,
    pub class_no: Option<i32>,
    pub class_label: String,
    pub status: String,
    pub transfer_in_date: Option<String>,
    pub transfer_out_date: Option<String>,
    pub transfer_out_note: Option<String>,
    pub graduated: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventRow {
    pub id: i64,
    pub school_year: i32,
    pub kind: String,
    pub kind_label: String,
    pub event_date: Option<String>,
    pub class_label: Option<String>,
    pub class_no: Option<i32>,
    pub note: Option<String>,
    pub created_at: String,
}

pub fn event_label(kind: &str) -> &'static str {
    match kind {
        "ENROLL" => "등록",
        "TRANSFER_IN" => "전입",
        "TRANSFER_OUT" => "전출",
        "PROMOTE" => "진급",
        "GRADUATE" => "졸업",
        "CANCEL" => "취소",
        _ => "기타",
    }
}

// ---------------------------------------------------------------
// 만들기 · 고치기
// ---------------------------------------------------------------

/// 학생이 **어떻게 들어왔는지**. 첫 사건을 무엇으로 남길지 정한다.
///
/// 처음 등록은 `ENROLL`, 전입은 `TRANSFER_IN`, 이미 나간 지난 전출생을 뒤늦게 적어
/// 넣는 것은 `TRANSFER_OUT` 이다. 어느 쪽이든 학생정보 정규화·주소 판정·확인 필요는
/// **이 함수 하나**를 지나므로 규칙이 갈라지지 않는다.
#[derive(Debug, Clone)]
pub struct Entry<'a> {
    /// 만들어질 학적 상태
    pub status: &'a str,
    /// 남길 사건 종류
    pub kind: &'a str,
    /// 사건이 일어난 날 (`YYYY-MM-DD`). 모르면 None
    pub date: Option<&'a str>,
    pub note: Option<&'a str>,
    pub source: &'a str,
}

impl Entry<'static> {
    /// 그냥 등록 — 지금까지의 모든 호출이 쓰던 값.
    pub fn enrolled() -> Self {
        Entry {
            status: "ENROLLED",
            kind: "ENROLL",
            date: None,
            note: None,
            source: "MANUAL",
        }
    }
}

/// 학생과 그 학년도 학적을 함께 만든다. `ENROLL` 사건을 남긴다.
pub fn create(c: &Connection, input: &StudentInput, today: NaiveDate) -> AppResult<i64> {
    create_with(c, input, today, &Entry::enrolled())
}

/// 들어온 경로를 정해서 학생을 만든다.
pub fn create_with(
    c: &Connection,
    input: &StudentInput,
    today: NaiveDate,
    entry: &Entry<'_>,
) -> AppResult<i64> {
    input.validate()?;
    require_year(c, input.school_year)?;

    let name = input.name.trim();
    let b = birth::parse(input.birth_raw.as_deref().unwrap_or(""), today);
    let (father_phone, father_digits) = phone::normalize(input.father_phone.as_deref().unwrap_or(""));
    let (mother_phone, mother_digits) = phone::normalize(input.mother_phone.as_deref().unwrap_or(""));
    let (primary_phone, primary_digits) =
        phone::normalize(input.primary_phone.as_deref().unwrap_or(""));
    let address_raw = clean(&input.address_raw);
    let parsed = address::parse(address_raw.as_deref().unwrap_or(""));
    let address_norm = (!parsed.norm.is_empty()).then(|| parsed.norm.clone());

    c.execute(
        "INSERT INTO students(
            name, gender, birth_raw, birth_date, address_raw, address_norm,
            father_name, mother_name,
            father_phone, father_phone_digits, mother_phone, mother_phone_digits,
            primary_phone, primary_phone_digits, note)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
        params![
            name,
            clean(&input.gender),
            clean(&input.birth_raw),
            b.date.map(birth::format_iso),
            address_raw,
            address_norm,
            clean(&input.father_name),
            clean(&input.mother_name),
            father_phone,
            father_digits,
            mother_phone,
            mother_digits,
            primary_phone,
            primary_digits,
            clean(&input.note),
        ],
    )?;
    let student_id = c.last_insert_rowid();

    // 주소 분류는 등록하자마자 따진다. 규칙이 이미 있으면 바로 분류된다.
    let book = addr_repo::Book::load(c)?;
    let decision = book.classify(&parsed);
    addr_repo::write_decision(c, student_id, &parsed, &decision)?;

    let class_name = clean(&input.class_name);
    c.execute(
        "INSERT INTO enrollments(student_id, school_year, grade, class_name, class_no, status)
         VALUES (?1,?2,?3,?4,?5,?6)",
        params![
            student_id,
            input.school_year,
            input.grade,
            class_name,
            input.class_no,
            entry.status
        ],
    )?;

    add_event(
        c,
        student_id,
        input.school_year,
        entry.kind,
        entry.date,
        input.grade,
        class_name.as_deref(),
        input.class_no,
        entry.note,
        entry.source,
    )?;

    sync_issues(c, student_id, input.school_year, today)?;

    // 이 번호를 이미 쓰던 학생이 있으면 그쪽 표시도 함께 켜야 한다
    let numbers: Vec<i32> = input.class_no.into_iter().collect();
    sync_number_peers(
        c,
        input.school_year,
        input.grade,
        class_name.as_deref(),
        &numbers,
        student_id,
        today,
    )?;
    Ok(student_id)
}

/// 기본정보와 그 학년도 학적을 함께 고친다.
pub fn update(
    c: &Connection,
    student_id: i64,
    input: &StudentInput,
    today: NaiveDate,
) -> AppResult<()> {
    input.validate()?;
    require_year(c, input.school_year)?;

    let name = input.name.trim();
    let b = birth::parse(input.birth_raw.as_deref().unwrap_or(""), today);
    let (father_phone, father_digits) = phone::normalize(input.father_phone.as_deref().unwrap_or(""));
    let (mother_phone, mother_digits) = phone::normalize(input.mother_phone.as_deref().unwrap_or(""));
    let (primary_phone, primary_digits) =
        phone::normalize(input.primary_phone.as_deref().unwrap_or(""));
    let address_raw = clean(&input.address_raw);
    let parsed = address::parse(address_raw.as_deref().unwrap_or(""));
    let address_norm = (!parsed.norm.is_empty()).then(|| parsed.norm.clone());

    let (old_address, old_source): (Option<String>, String) = c.query_row(
        "SELECT address_raw, address_source FROM students WHERE id = ?1",
        [student_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let address_changed = old_address != address_raw;

    let changed = c.execute(
        "UPDATE students SET
            name = ?2, gender = ?3, birth_raw = ?4, birth_date = ?5,
            address_raw = ?6, address_norm = ?7,
            father_name = ?8, mother_name = ?9,
            father_phone = ?10, father_phone_digits = ?11,
            mother_phone = ?12, mother_phone_digits = ?13,
            primary_phone = ?14, primary_phone_digits = ?15,
            note = ?16,
            updated_at = datetime('now','localtime')
          WHERE id = ?1",
        params![
            student_id,
            name,
            clean(&input.gender),
            clean(&input.birth_raw),
            b.date.map(birth::format_iso),
            address_raw,
            address_norm,
            clean(&input.father_name),
            clean(&input.mother_name),
            father_phone,
            father_digits,
            mother_phone,
            mother_digits,
            primary_phone,
            primary_digits,
            clean(&input.note),
        ],
    )?;
    if changed == 0 {
        return Err(AppError::not_found("학생을 찾을 수 없습니다."));
    }

    // 주소 분류 다시 따지기.
    //
    // 주소가 그대로면 이미 정해진 것을 그대로 둔다. 주소가 달라졌다면 예전 판정이
    // 새 주소에도 맞다고 볼 수 없으므로 다시 따진다 — 직접 지정(MANUAL)도 마찬가지다.
    // 사용자가 "직접 지정을 그대로 두겠다" 고 고른 경우에만 남긴다.
    let keep_manual = old_source == "MANUAL" && (!address_changed || input.keep_manual_address);
    if !keep_manual {
        let book = addr_repo::Book::load(c)?;
        let decision = book.classify(&parsed);
        addr_repo::write_decision(c, student_id, &parsed, &decision)?;
    } else {
        // 분류는 그대로 두고 정리값만 새 주소에 맞춘다
        c.execute(
            "UPDATE students SET address_norm = ?2, address_road = ?3 WHERE id = ?1",
            params![student_id, address_norm.as_deref(), parsed.road.as_deref()],
        )?;
    }

    let class_name = clean(&input.class_name);

    // 번호가 겹치던 상대를 찾으려면 바뀌기 전 자리를 알아야 한다
    let before: Option<(i32, Option<String>, Option<i32>)> = c
        .query_row(
            "SELECT grade, class_name, class_no FROM enrollments
              WHERE student_id = ?1 AND school_year = ?2",
            params![student_id, input.school_year],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;

    let n = c.execute(
        "UPDATE enrollments
            SET grade = ?3, class_name = ?4, class_no = ?5,
                updated_at = datetime('now','localtime')
          WHERE student_id = ?1 AND school_year = ?2",
        params![
            student_id,
            input.school_year,
            input.grade,
            class_name,
            input.class_no
        ],
    )?;
    if n == 0 {
        return Err(AppError::not_found(format!(
            "이 학생에게는 {}학년도 학적이 없습니다.",
            input.school_year
        )));
    }

    sync_issues_with_siblings(c, student_id, input.school_year, today)?;

    // 떠나온 자리와 새로 앉은 자리 양쪽의 번호 이웃을 다시 따진다
    if let Some((old_grade, old_class, old_no)) = before {
        let old_numbers: Vec<i32> = old_no.into_iter().collect();
        sync_number_peers(
            c,
            input.school_year,
            old_grade,
            old_class.as_deref(),
            &old_numbers,
            student_id,
            today,
        )?;
    }
    let new_numbers: Vec<i32> = input.class_no.into_iter().collect();
    sync_number_peers(
        c,
        input.school_year,
        input.grade,
        class_name.as_deref(),
        &new_numbers,
        student_id,
        today,
    )?;
    Ok(())
}

/// 그 학년도 학적이 없으면 만든다. 이미 있으면 아무것도 하지 않는다.
///
/// 지난 학년도에만 있던 학생이 올해 명단에 나타났을 때 쓴다 (가져오기·전환).
/// 만들 때는 `ENROLL` 사건도 함께 남긴다.
pub fn ensure_enrollment(
    c: &Connection,
    student_id: i64,
    school_year: i32,
    grade: i32,
    class_name: Option<&str>,
    class_no: Option<i32>,
    source: &str,
) -> AppResult<bool> {
    let exists: i64 = c.query_row(
        "SELECT COUNT(*) FROM enrollments WHERE student_id = ?1 AND school_year = ?2",
        params![student_id, school_year],
        |r| r.get(0),
    )?;
    if exists > 0 {
        return Ok(false);
    }

    c.execute(
        "INSERT INTO enrollments(student_id, school_year, grade, class_name, class_no, status)
         VALUES (?1,?2,?3,?4,?5,'ENROLLED')",
        params![student_id, school_year, grade, class_name, class_no],
    )?;
    add_event(
        c,
        student_id,
        school_year,
        "ENROLL",
        None,
        grade,
        class_name,
        class_no,
        None,
        source,
    )?;
    Ok(true)
}

/// 입력 실수를 되돌리기 위한 삭제. 학생과 딸린 자료가 모두 사라진다.
///
/// 졸업 기록이 있는 학생은 막는다 — 지난 자료를 실수로 지우는 일이 없도록.
/// 전출·진급 같은 정상적인 흐름에서는 삭제하지 않고 상태만 바꾼다.
pub fn delete(c: &Connection, student_id: i64) -> AppResult<()> {
    let graduated: i64 = c.query_row(
        "SELECT COUNT(*) FROM graduations WHERE student_id = ?1",
        [student_id],
        |r| r.get(0),
    )?;
    if graduated > 0 {
        return Err(AppError::new(
            "IN_USE",
            "졸업 기록이 있는 학생은 삭제할 수 없습니다. 졸업생 화면에서 확인해 주세요.",
        ));
    }
    let n = c.execute("DELETE FROM students WHERE id = ?1", [student_id])?;
    if n == 0 {
        return Err(AppError::not_found("학생을 찾을 수 없습니다."));
    }
    Ok(())
}

fn require_year(c: &Connection, year: i32) -> AppResult<()> {
    let n: i64 = c.query_row(
        "SELECT COUNT(*) FROM school_years WHERE year = ?1",
        [year],
        |r| r.get(0),
    )?;
    if n == 0 {
        return Err(AppError::setup_required(format!(
            "{year}학년도가 없습니다. 설정에서 학년도를 먼저 만들어 주세요."
        )));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn add_event(
    c: &Connection,
    student_id: i64,
    school_year: i32,
    kind: &str,
    event_date: Option<&str>,
    grade: i32,
    class_name: Option<&str>,
    class_no: Option<i32>,
    note: Option<&str>,
    source: &str,
) -> AppResult<()> {
    c.execute(
        "INSERT INTO enrollment_events(
            student_id, school_year, kind, event_date, grade, class_name, class_no, note, source)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![
            student_id,
            school_year,
            kind,
            event_date,
            grade,
            class_name,
            class_no,
            note,
            source
        ],
    )?;
    Ok(())
}

// ---------------------------------------------------------------
// 확인 필요 다시 계산
// ---------------------------------------------------------------

/// 한 학생의 자동 확인 필요를 다시 맞춘다. 문제가 사라졌으면 닫힌다.
///
/// Phase 1 에서 스스로 판단할 수 있는 것만 다룬다. 주소·형제는 각 Phase 에서
/// 같은 자리에 더한다.
pub fn sync_issues(
    c: &Connection,
    student_id: i64,
    school_year: i32,
    today: NaiveDate,
) -> AppResult<()> {
    let (name, gender, birth_raw, birth_date, address, has_phone): (
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        bool,
    ) = c.query_row(
        "SELECT name, gender, birth_raw, birth_date, address_raw,
                (father_phone_digits IS NOT NULL
                 OR mother_phone_digits IS NOT NULL
                 OR primary_phone_digits IS NOT NULL)
           FROM students WHERE id = ?1",
        [student_id],
        |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get::<_, i64>(5)? == 1,
            ))
        },
    )?;

    // --- 생년월일 ---
    let b = birth::parse(birth_raw.as_deref().unwrap_or(""), today);
    issue::set(
        c,
        student_id,
        IssueKind::Birth,
        b.problem.as_ref().map(|p| {
            (
                p.message(),
                // 원본을 함께 보여 줘야 무엇을 고쳐야 할지 안다
                birth_raw.as_ref().map(|r| format!("입력값: {r}")),
            )
        }),
    )?;

    // --- 필수 정보 누락 ---
    let mut missing: Vec<&str> = Vec::new();
    if gender.is_none() {
        missing.push("성별");
    }
    if birth_date.is_none() && b.problem.is_none() {
        missing.push("생년월일");
    }
    if address.is_none() {
        missing.push("주소");
    }
    if !has_phone {
        missing.push("보호자 연락처");
    }
    let missing_msg = if missing.is_empty() {
        None
    } else {
        Some(korean::list_with_subject(&missing, "비어 있습니다."))
    };
    issue::set(
        c,
        student_id,
        IssueKind::Missing,
        missing_msg.map(|m| (m, None)),
    )?;

    // --- 반·번호 미정 ---
    let (grade, class_name, class_no, status): (i32, Option<String>, Option<i32>, String) = c
        .query_row(
            "SELECT grade, class_name, class_no, status
               FROM enrollments WHERE student_id = ?1 AND school_year = ?2",
            params![student_id, school_year],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )?;
    let mut unset: Vec<&str> = Vec::new();
    if class_name.as_deref().map(str::trim).unwrap_or("").is_empty() {
        unset.push("반");
    }
    if class_no.is_none() {
        unset.push("번호");
    }
    let class_msg = if unset.is_empty() {
        None
    } else {
        Some(korean::list_with_subject(&unset, "정해지지 않았습니다."))
    };
    issue::set(
        c,
        student_id,
        IssueKind::ClassAssign,
        class_msg.map(|m| (m, None)),
    )?;

    // --- 같은 반 번호 중복 ---
    //
    // 번호에는 UNIQUE 제약을 걸지 않는다(걸면 엑셀 가져오기와 학기 초 정리가 막힌다).
    // 대신 겹치면 여기에 표시하고, 자동 번호 재정렬은 이 표시가 있는 반에서 멈춘다.
    let active = status != "TRANSFER_OUT";
    let dup_count: i64 = match class_no {
        Some(no) if active => c.query_row(
            &format!(
            "SELECT COUNT(*) FROM enrollments
              WHERE school_year = ?1 AND grade = ?2
                AND ((class_name IS NULL AND ?3 IS NULL) OR class_name = ?3)
                AND class_no = ?4
                AND {ACTIVE}"
            ),
            params![school_year, grade, class_name, no],
            |r| r.get(0),
        )?,
        _ => 0,
    };
    let number_problem = (dup_count > 1).then(|| {
        let where_ = label::class_label(grade, class_name.as_deref());
        (
            format!(
                "{where_}반에서 {}번을 쓰는 학생이 {dup_count}명 있습니다. 번호를 정리해 주세요.",
                class_no.unwrap_or(0)
            ),
            None,
        )
    });
    issue::set(c, student_id, IssueKind::NumberDup, number_problem)?;

    // --- 같은 학년도에 이름+생년월일이 같은 학생 ---
    let dup: Option<i64> = if birth_date.is_some() {
        c.query_row(
            "SELECT s.id FROM students s
               JOIN enrollments e ON e.student_id = s.id AND e.school_year = ?2
              WHERE s.id <> ?1 AND s.name = ?3 AND s.birth_date = ?4
              LIMIT 1",
            params![student_id, school_year, name, birth_date],
            |r| r.get(0),
        )
        .optional()?
    } else {
        None
    };
    match dup {
        Some(other) => issue::open(
            c,
            student_id,
            IssueKind::Duplicate,
            "같은 학년도에 이름과 생년월일이 같은 학생이 있습니다. 같은 학생인지 확인해 주세요.",
            Some(&format!("{{\"otherStudentId\":{other}}}")),
            None,
        )?,
        None => issue::close(c, student_id, IssueKind::Duplicate)?,
    }

    // --- 주소 분류 ---
    //
    // 주소가 있는데 분류하지 못했거나 규칙이 부딪히면 확인을 요청한다.
    // 분류가 되면(직접 지정 포함) 표시는 저절로 닫힌다.
    // 주소 자체가 없는 것은 위의 '필수 정보 누락' 이 이미 알린다.
    let (addr_source, has_category): (String, bool) = c.query_row(
        "SELECT address_source, address_category_id IS NOT NULL FROM students WHERE id = ?1",
        [student_id],
        |r| Ok((r.get(0)?, r.get::<_, i64>(1)? == 1)),
    )?;

    let addr_problem = if address.is_none() || has_category {
        None
    } else if addr_source == "CONFLICT" {
        let detail = c
            .query_row(
                "SELECT address_raw FROM students WHERE id = ?1",
                [student_id],
                |r| r.get::<_, Option<String>>(0),
            )
            .ok()
            .flatten()
            .map(|raw| {
                let parsed = address::parse(&raw);
                match addr_repo::Book::load(c).map(|b| b.classify(&parsed)) {
                    Ok(d) => d.reason(),
                    Err(_) => String::new(),
                }
            })
            .filter(|s| !s.is_empty());
        Some((
            "주소 규칙이 서로 다른 분류를 가리킵니다. 규칙을 확인해 주세요.".to_string(),
            detail,
        ))
    } else {
        Some((
            "주소 분류를 정하지 못했습니다. 분류를 지정하거나 주소 규칙을 만들어 주세요."
                .to_string(),
            None,
        ))
    };
    issue::set(c, student_id, IssueKind::Address, addr_problem)?;

    // --- 형제 ---
    //
    // 상대가 여럿일 수 있으므로 관계마다 한 줄씩 띄우고(`ref_id` = 관계 번호),
    // 더는 해당하지 않는 줄만 닫는다.
    //
    // 형제 관계와 보호자 정보 일치는 다른 문제다. 연락처가 달라도 형제일 수 있으므로
    // 확정 관계를 풀지 않고 '확인해 달라' 고만 한다.
    let mine = sibling_repo::guardians_of(c, student_id)?;
    let mut keep_candidate: Vec<i64> = Vec::new();
    let mut keep_fill: Vec<i64> = Vec::new();
    let mut keep_conflict: Vec<i64> = Vec::new();

    for link in sibling_repo::links_of(c, student_id)? {
        let other = link.other(student_id);
        match link.status.as_str() {
            "CANDIDATE" => {
                let label = sibling_repo::student_label_of(c, other, school_year)?;
                issue::open(
                    c,
                    student_id,
                    IssueKind::SiblingCandidate,
                    &format!("{label} 학생과 형제인지 확인해 주세요."),
                    Some(&format!("{{\"otherStudentId\":{other}}}")),
                    Some(link.id),
                )?;
                keep_candidate.push(link.id);
            }
            "CONFIRMED" => {
                let theirs = sibling_repo::guardians_of(c, other)?;
                let label = sibling_repo::student_label_of(c, other, school_year)?;

                // 형제에게는 있고 나에게는 없는 항목 — 가져올 수 있다
                let fillable = sibling::fillable(&mine, &theirs);
                if !fillable.is_empty() {
                    let names: Vec<&str> = fillable.iter().map(|f| f.label()).collect();
                    issue::open(
                        c,
                        student_id,
                        IssueKind::GuardianFill,
                        &format!(
                            "형제 {label} 학생에게 등록된 {}을(를) 가져올 수 있습니다.",
                            names.join(", ")
                        ),
                        Some(&sibling::to_json(&fillable)),
                        Some(link.id),
                    )?;
                    keep_fill.push(link.id);
                }

                // 양쪽 다 값이 있는데 다른 항목 — 어느 쪽이 맞는지 정하지 않는다
                let conflicts = sibling::compare(&mine, &theirs).conflicts;
                if !conflicts.is_empty() {
                    let names: Vec<&str> = conflicts.iter().map(|f| f.label()).collect();
                    issue::open(
                        c,
                        student_id,
                        IssueKind::GuardianConflict,
                        &format!("형제 {label} 학생과 {}이(가) 서로 다릅니다.", names.join(", ")),
                        Some(&sibling::to_json(&conflicts)),
                        Some(link.id),
                    )?;
                    keep_conflict.push(link.id);
                }
            }
            _ => {} // REJECTED — 사용자가 이미 정했으므로 다시 묻지 않는다
        }
    }

    issue::close_except(c, student_id, IssueKind::SiblingCandidate, &keep_candidate)?;
    issue::close_except(c, student_id, IssueKind::GuardianFill, &keep_fill)?;
    issue::close_except(c, student_id, IssueKind::GuardianConflict, &keep_conflict)?;

    Ok(())
}

/// 같은 반에서 이 번호들을 쓰는 다른 학생의 표시도 다시 맞춘다.
///
/// 번호 중복은 **둘 이상이 함께 겪는 문제**다. 한쪽이 번호를 비켜 주면 남은 쪽의
/// 표시도 사라져야 하는데, 고친 학생만 다시 따지면 상대 쪽 표시가 남는다.
pub fn sync_number_peers(
    c: &Connection,
    school_year: i32,
    grade: i32,
    class_name: Option<&str>,
    numbers: &[i32],
    skip: i64,
    today: NaiveDate,
) -> AppResult<()> {
    for no in numbers {
        let ids: Vec<i64> = c
            .prepare_cached(
                "SELECT student_id FROM enrollments
                  WHERE school_year = ?1 AND grade = ?2
                    AND ((class_name IS NULL AND ?3 IS NULL) OR class_name = ?3)
                    AND class_no = ?4 AND student_id <> ?5",
            )?
            .query_map(params![school_year, grade, class_name, no, skip], |r| {
                r.get(0)
            })?
            .collect::<rusqlite::Result<_>>()?;
        for id in ids {
            sync_issues(c, id, school_year, today)?;
        }
    }
    Ok(())
}

/// 이 학생과 **관계가 있는 학생까지** 확인 필요를 다시 맞춘다.
///
/// 보호자 정보를 고치면 상대 쪽 표시도 달라진다. A 의 연락처를 고쳐 B 와 같아졌으면
/// B 의 '정보 불일치' 도 닫혀야 한다. 한 단계만 따라간다(되돌이 없음).
pub fn sync_issues_with_siblings(
    c: &Connection,
    student_id: i64,
    school_year: i32,
    today: NaiveDate,
) -> AppResult<()> {
    let partners: Vec<i64> = sibling_repo::links_of(c, student_id)?
        .into_iter()
        .map(|l| l.other(student_id))
        .collect();

    sync_issues(c, student_id, school_year, today)?;
    for other in partners {
        // 상대가 그 학년도에 없으면(전출·졸업) 건너뛴다
        let in_year: i64 = c.query_row(
            "SELECT COUNT(*) FROM enrollments WHERE student_id = ?1 AND school_year = ?2",
            params![other, school_year],
            |r| r.get(0),
        )?;
        if in_year > 0 {
            sync_issues(c, other, school_year, today)?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------
// 조회
// ---------------------------------------------------------------

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListFilter {
    pub school_year: i32,
    /// 통합 검색 — 이름·주소·연락처·번호를 한 칸에서 찾는다
    pub q: Option<String>,
    pub grade: Option<i32>,
    pub class_name: Option<String>,
    pub address_category_id: Option<i64>,
    /// 주소는 있는데 아직 분류하지 못한 학생만
    #[serde(default)]
    pub address_unclassified: bool,
    /// 주소 자체가 없는 학생만. 통계의 '주소 없음' 을 눌렀을 때 쓴다.
    #[serde(default)]
    pub address_none: bool,
    /// ACTIVE(기본, 재학+전입) / ALL / ENROLLED / TRANSFER_IN / TRANSFER_OUT
    pub status: Option<String>,
    // 항목별 검색
    pub name: Option<String>,
    pub class_no: Option<i32>,
    pub address: Option<String>,
    pub father_phone: Option<String>,
    pub mother_phone: Option<String>,
    pub primary_phone: Option<String>,
    /// 확인 필요가 있는 학생만
    #[serde(default)]
    pub only_issues: bool,
}

struct Where {
    sql: Vec<String>,
    args: Vec<Value>,
}

impl Where {
    fn push(&mut self, cond: &str, value: Value) {
        self.sql.push(cond.to_string());
        self.args.push(value);
    }
    fn like(&mut self, column: &str, term: &str) {
        self.sql.push(format!("{column} LIKE ?"));
        self.args.push(Value::Text(format!("%{term}%")));
    }
}

/// 연락처 부분 검색에 필요한 최소 자릿수. 이보다 짧으면 온 학교가 걸린다.
const PHONE_MIN: usize = 3;

/// 내보내기도 **같은 조건**으로 학생을 뽑는다. 명단에서 48명이 보였는데 파일에
/// 47명이 들어 있으면 둘 다 못 쓴다.
pub fn build_where_pub(f: &ListFilter) -> (Vec<String>, Vec<Value>) {
    let w = build_where(f);
    (w.sql, w.args)
}

fn build_where(f: &ListFilter) -> Where {
    let mut w = Where {
        sql: vec!["e.school_year = ?".into()],
        args: vec![Value::Integer(f.school_year as i64)],
    };

    match f.status.as_deref().unwrap_or("ACTIVE") {
        "ALL" => {}
        "ACTIVE" => w.sql.push(format!("e.{ACTIVE}")),
        other => w.push("e.status = ?", Value::Text(other.to_string())),
    }

    if let Some(g) = f.grade {
        w.push("e.grade = ?", Value::Integer(g as i64));
    }
    if let Some(cn) = f.class_name.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        w.push("e.class_name = ?", Value::Text(cn.to_string()));
    }
    if let Some(cat) = f.address_category_id {
        w.push("s.address_category_id = ?", Value::Integer(cat));
    }
    // 주소 상태는 통계와 **같은 조건**을 쓴다. 통계에서 숫자를 누르고 넘어왔을 때
    // 명단에 뜨는 인원이 그 숫자와 달라지면 어느 쪽도 믿을 수 없게 된다.
    if f.address_unclassified {
        w.sql.push(stats::UNCLASSIFIED_SQL.into());
    }
    if f.address_none {
        w.sql.push(stats::NO_ADDRESS_SQL.into());
    }
    if let Some(n) = f.class_no {
        w.push("e.class_no = ?", Value::Integer(n as i64));
    }
    if let Some(t) = f.name.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        w.like("s.name", t);
    }
    if let Some(t) = f.address.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        w.like("s.address_norm", t);
    }
    for (term, column) in [
        (&f.father_phone, "s.father_phone_digits"),
        (&f.mother_phone, "s.mother_phone_digits"),
        (&f.primary_phone, "s.primary_phone_digits"),
    ] {
        if let Some(t) = term.as_deref().filter(|s| !s.trim().is_empty()) {
            // 하이픈을 지우고 숫자로만 견준다 — 1234 로도 5678 로도 찾힌다
            match phone::search_digits(t, PHONE_MIN) {
                Some(d) => w.like(column, &d),
                // 숫자가 너무 짧으면 아무것도 찾지 않는다 (온 학교가 걸리는 것보다 낫다)
                None => w.sql.push("0 = 1".into()),
            }
        }
    }
    if f.only_issues {
        w.sql
            .push("EXISTS (SELECT 1 FROM issues i WHERE i.student_id = s.id AND i.status = 'OPEN')".into());
    }

    // 통합 검색 — 이름·주소는 글자로, 연락처는 숫자로, 짧은 숫자는 번호로 본다
    if let Some(q) = f.q.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        let mut or: Vec<String> = Vec::new();
        let mut args: Vec<Value> = Vec::new();
        let digits = phone::digits(q);

        // 숫자만 친 검색어로는 이름·주소를 뒤지지 않는다.
        // '2' 로 찾았을 때 주소에 123 이 들어간 집이 줄줄이 걸리면 안 된다.
        if digits.len() != q.chars().count() {
            or.push("s.name LIKE ?".into());
            args.push(Value::Text(format!("%{q}%")));
            or.push("s.address_norm LIKE ?".into());
            args.push(Value::Text(format!("%{q}%")));
        }

        if digits.len() >= PHONE_MIN {
            for col in [
                "s.father_phone_digits",
                "s.mother_phone_digits",
                "s.primary_phone_digits",
            ] {
                or.push(format!("{col} LIKE ?"));
                args.push(Value::Text(format!("%{digits}%")));
            }
        }
        // 1~99 는 출석번호로도 본다
        if let Ok(n) = digits.parse::<i64>() {
            if (1..=99).contains(&n) && digits.len() <= 2 {
                or.push("e.class_no = ?".into());
                args.push(Value::Integer(n));
            }
        }

        // 어느 칸으로도 찾을 수 없는 검색어면 아무도 내놓지 않는다
        if or.is_empty() {
            w.sql.push("0 = 1".into());
        } else {
            w.sql.push(format!("({})", or.join(" OR ")));
            w.args.extend(args);
        }
    }

    w
}

const SELECT_ROW: &str = "
    SELECT s.id, s.name, s.gender, s.birth_date, s.birth_raw,
           e.grade, e.class_name, e.class_no, e.status,
           s.address_raw, ac.name,
           s.father_phone, s.mother_phone, s.primary_phone,
           (SELECT COUNT(*) FROM issues i WHERE i.student_id = s.id AND i.status = 'OPEN'),
           (SELECT COUNT(*) FROM graduations g
             WHERE g.student_id = s.id AND g.school_year = e.school_year)
      FROM enrollments e
      JOIN students s ON s.id = e.student_id
      LEFT JOIN address_categories ac ON ac.id = s.address_category_id";

fn map_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<StudentRow> {
    let grade: i32 = r.get(5)?;
    let class_name: Option<String> = r.get(6)?;
    Ok(StudentRow {
        id: r.get(0)?,
        name: r.get(1)?,
        gender: r.get(2)?,
        birth_date: r.get(3)?,
        birth_raw: r.get(4)?,
        class_label: label::class_label(grade, class_name.as_deref()),
        grade,
        class_name,
        class_no: r.get(7)?,
        status: r.get(8)?,
        address_raw: r.get(9)?,
        address_category: r.get(10)?,
        father_phone: r.get(11)?,
        mother_phone: r.get(12)?,
        primary_phone: r.get(13)?,
        issue_count: r.get(14)?,
        graduated: r.get::<_, i64>(15)? > 0,
        // 형제 표시는 목록을 읽은 뒤에 채운다 (지금 학적으로 이름표를 만들어야 한다)
        sibling: None,
    })
}

pub struct ListPage {
    pub rows: Vec<StudentRow>,
    pub total: i64,
}

/// 학생명단. 기본 정렬은 학년 → 반 → 번호 → 이름.
pub fn list(c: &Connection, f: &ListFilter, limit: i64, offset: i64) -> AppResult<ListPage> {
    let w = build_where(f);
    let where_sql = w.sql.join(" AND ");

    let total: i64 = c.query_row(
        &format!(
            "SELECT COUNT(*) FROM enrollments e
               JOIN students s ON s.id = e.student_id
              WHERE {where_sql}"
        ),
        params_from_iter(w.args.iter()),
        |r| r.get(0),
    )?;

    let sql = format!(
        "{SELECT_ROW} WHERE {where_sql} ORDER BY {} LIMIT ? OFFSET ?",
        label::ORDER_BY_ROSTER
    );
    let mut args = w.args.clone();
    args.push(Value::Integer(limit));
    args.push(Value::Integer(offset));

    let mut st = c.prepare(&sql)?;
    let mut rows = st
        .query_map(params_from_iter(args.iter()), map_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(st);

    // 형제 이름표는 저장해 두지 않는다 — 진급하면 바뀌어야 하므로 볼 때마다 만든다
    for row in rows.iter_mut() {
        row.sibling = sibling_repo::brief(c, row.id, f.school_year)?;
    }

    Ok(ListPage { rows, total })
}

/// 학생 한 명의 모든 것 — Drawer 가 쓴다.
pub fn detail(c: &Connection, student_id: i64, school_year: i32) -> AppResult<StudentDetail> {
    let mut d = c
        .query_row(
            "SELECT s.id, s.name, s.gender, s.birth_raw, s.birth_date,
                    s.address_raw, s.address_category_id, ac.name, s.address_source,
                    s.father_name, s.mother_name,
                    s.father_phone, s.mother_phone, s.primary_phone,
                    s.note, s.created_at, s.updated_at
               FROM students s
               LEFT JOIN address_categories ac ON ac.id = s.address_category_id
              WHERE s.id = ?1",
            [student_id],
            |r| {
                Ok(StudentDetail {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    gender: r.get(2)?,
                    birth_raw: r.get(3)?,
                    birth_date: r.get(4)?,
                    address_raw: r.get(5)?,
                    address_category_id: r.get(6)?,
                    address_category: r.get(7)?,
                    address_source: r.get(8)?,
                    father_name: r.get(9)?,
                    mother_name: r.get(10)?,
                    father_phone: r.get(11)?,
                    mother_phone: r.get(12)?,
                    primary_phone: r.get(13)?,
                    note: r.get(14)?,
                    created_at: r.get(15)?,
                    updated_at: r.get(16)?,
                    enrollment: None,
                    enrollments: Vec::new(),
                    events: Vec::new(),
                    issues: Vec::new(),
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("학생을 찾을 수 없습니다."))?;

    d.enrollments = enrollments_of(c, student_id)?;
    d.enrollment = d
        .enrollments
        .iter()
        .find(|e| e.school_year == school_year)
        .cloned();
    d.events = events_of(c, student_id)?;
    d.issues = issue::list_for_student(c, student_id)?;
    Ok(d)
}

pub fn enrollments_of(c: &Connection, student_id: i64) -> AppResult<Vec<EnrollmentRow>> {
    let mut st = c.prepare(
        "SELECT e.school_year, e.grade, e.class_name, e.class_no, e.status,
                e.transfer_in_date, e.transfer_out_date, e.transfer_out_note,
                (SELECT COUNT(*) FROM graduations g
                  WHERE g.student_id = e.student_id AND g.school_year = e.school_year)
           FROM enrollments e
          WHERE e.student_id = ?1
          ORDER BY e.school_year DESC",
    )?;
    let rows = st
        .query_map([student_id], |r| {
            let grade: i32 = r.get(1)?;
            let class_name: Option<String> = r.get(2)?;
            Ok(EnrollmentRow {
                school_year: r.get(0)?,
                class_label: label::class_label(grade, class_name.as_deref()),
                grade,
                class_name,
                class_no: r.get(3)?,
                status: r.get(4)?,
                transfer_in_date: r.get(5)?,
                transfer_out_date: r.get(6)?,
                transfer_out_note: r.get(7)?,
                graduated: r.get::<_, i64>(8)? > 0,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

pub fn events_of(c: &Connection, student_id: i64) -> AppResult<Vec<EventRow>> {
    let mut st = c.prepare(
        "SELECT id, school_year, kind, event_date, grade, class_name, class_no, note, created_at
           FROM enrollment_events
          WHERE student_id = ?1
          ORDER BY school_year, id",
    )?;
    let rows = st
        .query_map([student_id], |r| {
            let kind: String = r.get(2)?;
            let grade: Option<i32> = r.get(4)?;
            let class_name: Option<String> = r.get(5)?;
            Ok(EventRow {
                id: r.get(0)?,
                school_year: r.get(1)?,
                kind_label: event_label(&kind).to_string(),
                kind,
                event_date: r.get(3)?,
                class_label: grade.map(|g| label::class_label(g, class_name.as_deref())),
                class_no: r.get(6)?,
                note: r.get(7)?,
                created_at: r.get(8)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassOption {
    pub grade: i32,
    pub class_name: String,
    pub count: i64,
}

/// 그 학년도에 실제로 있는 학년·반 목록. 검색 칸의 선택지로 쓴다.
pub fn class_options(c: &Connection, school_year: i32) -> AppResult<Vec<ClassOption>> {
    let mut st = c.prepare(&format!(
        "SELECT e.grade, e.class_name, COUNT(*)
           FROM enrollments e
          WHERE e.school_year = ?1
            AND e.class_name IS NOT NULL AND TRIM(e.class_name) <> ''
            AND e.{ACTIVE}
          GROUP BY e.grade, e.class_name",
    ))?;
    let mut rows: Vec<ClassOption> = st
        .query_map([school_year], |r| {
            Ok(ClassOption {
                grade: r.get(0)?,
                class_name: r.get(1)?,
                count: r.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    rows.sort_by(|a, b| {
        a.grade
            .cmp(&b.grade)
            .then_with(|| label::class_sort_key(Some(&a.class_name)).cmp(&label::class_sort_key(Some(&b.class_name))))
    });
    Ok(rows)
}

#[cfg(test)]
#[path = "student_tests.rs"]
mod student_tests;
