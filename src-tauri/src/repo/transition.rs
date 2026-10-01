//! 학년도 전환 저장소 — 읽기와 쓰기.
//!
//! **지난 학년도 학적은 건드리지 않는다.** UPDATE 가 한 줄도 없는 것이 이 모듈의
//! 핵심이다. 2026학년도 행은 그대로 두고 2027학년도 행을 새로 넣는다. 학생 번호
//! (`students.id`)는 그대로이므로 형제 관계·지난 이력이 모두 이어진다.

use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension};

use crate::domain::enroll;
use crate::domain::label;
use crate::domain::transition::{self, Seat};
use crate::error::{AppError, AppResult};
use crate::repo::student;

/// 원본 학년도 학생 한 명. 배정 자료와 짝지을 때 쓴다.
#[derive(Debug, Clone)]
pub struct SeatRow {
    pub student_id: i64,
    pub name: String,
    pub gender: Option<String>,
    /// 배정 자료와 짝지을 때 이름과 함께 본다
    pub birth_date: Option<String>,
    pub grade: i32,
    pub class_name: Option<String>,
    pub class_no: Option<i32>,
    pub status: String,
}

impl SeatRow {
    pub fn class_label(&self) -> String {
        label::class_label(self.grade, self.class_name.as_deref())
    }

    /// `3-가람-7` — 사람이 읽을 지금 자리.
    pub fn seat_label(&self) -> String {
        match self.class_no {
            Some(n) => format!("{}-{n}", self.class_label()),
            None => self.class_label(),
        }
    }

    fn seat(&self) -> Seat {
        Seat {
            student_id: self.student_id,
            grade: self.grade,
            class_name: self.class_name.clone(),
            class_no: self.class_no,
            status: self.status.clone(),
        }
    }
}

/// 전환 대상 학생 — 원본 학년도의 **현재 재학생**.
///
/// 지금 전출 상태인 학생은 들어오지 않는다. 나갔다가 돌아와 지금 다니는 학생은
/// 들어온다. 기준은 명단·통계와 같은 `ACTIVE_STATUS_SQL` 하나다.
pub fn seats(c: &Connection, from_year: i32, asof: NaiveDate) -> AppResult<Vec<SeatRow>> {
    let active = enroll::active_sql("e.", asof);
    let sql = format!(
        "SELECT e.student_id, s.name, s.gender, s.birth_date,
                e.grade, e.class_name, e.class_no, e.status
           FROM enrollments e
           JOIN students s ON s.id = e.student_id
          WHERE e.school_year = ?1 AND {active}
            AND NOT EXISTS (SELECT 1 FROM graduations g WHERE g.student_id = s.id)
          ORDER BY {order}",
        order = label::ORDER_BY_ROSTER,
    );
    let mut st = c.prepare(&sql)?;
    let rows = st
        .query_map([from_year], |r| {
            Ok(SeatRow {
                student_id: r.get(0)?,
                name: r.get(1)?,
                gender: r.get(2)?,
                birth_date: r.get(3)?,
                grade: r.get(4)?,
                class_name: r.get(5)?,
                class_no: r.get(6)?,
                status: r.get(7)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// 원본 학년도의 지금 상태. 미리보기를 만든 뒤 달라졌는지 보는 데 쓴다.
pub fn state_key(c: &Connection, from_year: i32, asof: NaiveDate) -> AppResult<String> {
    let seats: Vec<Seat> = seats(c, from_year, asof)?.iter().map(SeatRow::seat).collect();
    Ok(transition::state_key(&seats))
}

// ---------------------------------------------------------------
// 학년도 상태
// ---------------------------------------------------------------

/// 한 학년도가 지금 어떤 상태인가.
#[derive(Debug, Clone)]
pub struct YearState {
    pub year: i32,
    /// `school_years` 에 있는가
    pub exists: bool,
    /// 그 학년도 학적 수 (전출 포함)
    pub enrollments: i64,
    /// 그 학년도 재학생 수
    pub active: i64,
}

pub fn year_state(c: &Connection, year: i32, asof: NaiveDate) -> AppResult<YearState> {
    let active = enroll::active_sql("", asof);
    let exists: i64 = c.query_row(
        "SELECT COUNT(*) FROM school_years WHERE year = ?1",
        [year],
        |r| r.get(0),
    )?;
    let enrollments: i64 = c.query_row(
        "SELECT COUNT(*) FROM enrollments WHERE school_year = ?1",
        [year],
        |r| r.get(0),
    )?;
    let active: i64 = c.query_row(
        &format!("SELECT COUNT(*) FROM enrollments WHERE school_year = ?1 AND {active}"),
        [year],
        |r| r.get(0),
    )?;
    Ok(YearState {
        year,
        exists: exists > 0,
        enrollments,
        active,
    })
}

/// 이 두 학년도 사이 전환을 이미 돌린 적이 있는가.
pub fn done_before(c: &Connection, from_year: i32, to_year: i32) -> AppResult<Option<String>> {
    Ok(c.query_row(
        "SELECT executed_at FROM year_transitions
          WHERE from_year = ?1 AND to_year = ?2
          ORDER BY id DESC LIMIT 1",
        params![from_year, to_year],
        |r| r.get(0),
    )
    .optional()?)
}

// ---------------------------------------------------------------
// 쓰기 — 여기서도 지난 학년도는 건드리지 않는다
// ---------------------------------------------------------------

/// 다음 학년도 학적을 **새로** 만든다.
///
/// `UPDATE enrollments` 가 아니라 `INSERT` 다. 지난 학년도 학년·반·번호는 그대로 남는다.
pub fn promote(
    c: &Connection,
    student_id: i64,
    to_year: i32,
    grade: i32,
    class_name: Option<&str>,
    class_no: Option<i32>,
) -> AppResult<()> {
    if !transition::is_valid_grade(grade) {
        return Err(AppError::invalid(format!(
            "{grade}학년으로는 올릴 수 없습니다."
        )));
    }
    c.execute(
        "INSERT INTO enrollments(student_id, school_year, grade, class_name, class_no, status)
         VALUES (?1,?2,?3,?4,?5,'ENROLLED')",
        params![student_id, to_year, grade, class_name, class_no],
    )?;
    student::add_event(
        c,
        student_id,
        to_year,
        "PROMOTE",
        None,
        grade,
        class_name,
        class_no,
        None,
        "TRANSITION",
    )?;
    Ok(())
}

/// 졸업 기록을 만든다. 다음 학년도 학적은 만들지 않는다.
///
/// 졸업 당시 학년·반·번호는 원본 학년도 학적이 그대로 갖고 있고(고치지 않으므로),
/// 사건에도 그 시점 스냅샷으로 함께 남긴다.
pub fn graduate(
    c: &Connection,
    student_id: i64,
    from_year: i32,
    grade: i32,
    class_name: Option<&str>,
    class_no: Option<i32>,
    today: NaiveDate,
) -> AppResult<()> {
    c.execute(
        "INSERT INTO graduations(student_id, school_year, graduated_at)
         VALUES (?1,?2,?3)",
        params![student_id, from_year, today.to_string()],
    )?;
    student::add_event(
        c,
        student_id,
        from_year,
        "GRADUATE",
        Some(&today.to_string()),
        grade,
        class_name,
        class_no,
        None,
        "TRANSITION",
    )?;
    Ok(())
}

/// 전환 작업 자체를 남긴다. **개인정보는 넣지 않는다** — 인원과 학년도뿐이다.
pub fn record(c: &Connection, from_year: i32, to_year: i32, summary: &str) -> AppResult<i64> {
    c.execute(
        "INSERT INTO year_transitions(from_year, to_year, summary) VALUES (?1,?2,?3)",
        params![from_year, to_year, summary],
    )?;
    Ok(c.last_insert_rowid())
}

/// 지난 전환 기록.
#[derive(Debug, Clone)]
pub struct HistoryRow {
    pub id: i64,
    pub from_year: i32,
    pub to_year: i32,
    pub executed_at: String,
    pub summary: Option<String>,
}

pub fn history(c: &Connection, limit: i64) -> AppResult<Vec<HistoryRow>> {
    let mut st = c.prepare(
        "SELECT id, from_year, to_year, executed_at, summary
           FROM year_transitions ORDER BY id DESC LIMIT ?1",
    )?;
    let rows = st
        .query_map([limit], |r| {
            Ok(HistoryRow {
                id: r.get(0)?,
                from_year: r.get(1)?,
                to_year: r.get(2)?,
                executed_at: r.get(3)?,
                summary: r.get(4)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

#[cfg(test)]
#[path = "transition_tests.rs"]
mod transition_tests;
