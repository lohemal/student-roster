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

use crate::domain::{birth, label, phone};
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

/// 주소 검색·규칙 맞춤에 쓸 정리본. Phase 3 에서 더 다듬는다.
fn normalize_address(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
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

/// 학생과 그 학년도 학적을 함께 만든다. `ENROLL` 사건을 남긴다.
pub fn create(c: &Connection, input: &StudentInput, today: NaiveDate) -> AppResult<i64> {
    input.validate()?;
    require_year(c, input.school_year)?;

    let name = input.name.trim();
    let b = birth::parse(input.birth_raw.as_deref().unwrap_or(""), today);
    let (father_phone, father_digits) = phone::normalize(input.father_phone.as_deref().unwrap_or(""));
    let (mother_phone, mother_digits) = phone::normalize(input.mother_phone.as_deref().unwrap_or(""));
    let (primary_phone, primary_digits) =
        phone::normalize(input.primary_phone.as_deref().unwrap_or(""));
    let address_raw = clean(&input.address_raw);
    let address_norm = address_raw.as_deref().map(normalize_address);

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

    let class_name = clean(&input.class_name);
    c.execute(
        "INSERT INTO enrollments(student_id, school_year, grade, class_name, class_no, status)
         VALUES (?1,?2,?3,?4,?5,'ENROLLED')",
        params![
            student_id,
            input.school_year,
            input.grade,
            class_name,
            input.class_no
        ],
    )?;

    add_event(
        c,
        student_id,
        input.school_year,
        "ENROLL",
        None,
        input.grade,
        class_name.as_deref(),
        input.class_no,
        None,
        "MANUAL",
    )?;

    sync_issues(c, student_id, input.school_year, today)?;
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
    let address_norm = address_raw.as_deref().map(normalize_address);

    // 주소가 바뀌면 분류 근거를 다시 따져야 한다. 다만 사용자가 직접 지정한
    // 분류(MANUAL)는 건드리지 않는다 — 자동화가 사람 판단을 덮어쓰지 않는다.
    let old_address: Option<String> = c
        .query_row(
            "SELECT address_raw FROM students WHERE id = ?1",
            [student_id],
            |r| r.get(0),
        )
        .optional()?
        .flatten();
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

    if address_changed {
        c.execute(
            "UPDATE students
                SET address_category_id = NULL, address_source = 'NONE', address_rule_id = NULL
              WHERE id = ?1 AND address_source <> 'MANUAL'",
            [student_id],
        )?;
    }

    let class_name = clean(&input.class_name);
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

    sync_issues(c, student_id, input.school_year, today)?;
    Ok(())
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
        Some(format!("{}이(가) 비어 있습니다.", missing.join(", ")))
    };
    issue::set(
        c,
        student_id,
        IssueKind::Missing,
        missing_msg.map(|m| (m, None)),
    )?;

    // --- 반·번호 미정 ---
    let (class_name, class_no): (Option<String>, Option<i32>) = c.query_row(
        "SELECT class_name, class_no FROM enrollments WHERE student_id = ?1 AND school_year = ?2",
        params![student_id, school_year],
        |r| Ok((r.get(0)?, r.get(1)?)),
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
        Some(format!("{}이(가) 정해지지 않았습니다.", unset.join("· ")))
    };
    issue::set(
        c,
        student_id,
        IssueKind::ClassAssign,
        class_msg.map(|m| (m, None)),
    )?;

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

fn build_where(f: &ListFilter) -> Where {
    let mut w = Where {
        sql: vec!["e.school_year = ?".into()],
        args: vec![Value::Integer(f.school_year as i64)],
    };

    match f.status.as_deref().unwrap_or("ACTIVE") {
        "ALL" => {}
        "ACTIVE" => w.sql.push("e.status IN ('ENROLLED','TRANSFER_IN')".into()),
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
    let rows = st
        .query_map(params_from_iter(args.iter()), map_row)?
        .collect::<rusqlite::Result<Vec<_>>>()?;

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
          ORDER BY school_year DESC, id DESC",
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
    let mut st = c.prepare(
        "SELECT e.grade, e.class_name, COUNT(*)
           FROM enrollments e
          WHERE e.school_year = ?1
            AND e.class_name IS NOT NULL AND TRIM(e.class_name) <> ''
            AND e.status IN ('ENROLLED','TRANSFER_IN')
          GROUP BY e.grade, e.class_name",
    )?;
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
