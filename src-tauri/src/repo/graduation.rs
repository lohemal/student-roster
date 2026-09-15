//! 졸업생 저장소.
//!
//! 졸업생을 **따로 복사해 두지 않는다.** 학생 자료는 한 벌뿐이고, 졸업은 그 학생에게
//! 붙는 기록이다. 졸업 당시 학년·반·번호는 그 학년도 학적이 그대로 갖고 있다 —
//! 전환이 지난 학년도를 고치지 않으므로 나중에 반이 바뀌어 흐트러질 일이 없다.

use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension};

use crate::domain::label;
use crate::error::{AppError, AppResult};
use crate::repo::student;

/// 졸업 학년도 하나와 그 해 졸업생 수.
#[derive(Debug, Clone)]
pub struct GradYear {
    pub school_year: i32,
    pub count: i64,
}

pub fn years(c: &Connection) -> AppResult<Vec<GradYear>> {
    let mut st = c.prepare(
        "SELECT school_year, COUNT(*) FROM graduations
          GROUP BY school_year ORDER BY school_year DESC",
    )?;
    let rows = st
        .query_map([], |r| {
            Ok(GradYear {
                school_year: r.get(0)?,
                count: r.get(1)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// 졸업생 한 명 — 졸업 당시 자리 그대로.
#[derive(Debug, Clone)]
pub struct GradRow {
    pub student_id: i64,
    pub school_year: i32,
    pub name: String,
    pub gender: Option<String>,
    pub birth_date: Option<String>,
    pub birth_raw: Option<String>,
    /// 졸업 당시 학년 (학적이 없으면 None)
    pub grade: Option<i32>,
    pub class_name: Option<String>,
    pub class_no: Option<i32>,
    pub class_label: Option<String>,
    pub graduated_at: Option<String>,
    pub note: Option<String>,
}

fn map(r: &rusqlite::Row<'_>) -> rusqlite::Result<GradRow> {
    let grade: Option<i32> = r.get(6)?;
    let class_name: Option<String> = r.get(7)?;
    Ok(GradRow {
        student_id: r.get(0)?,
        school_year: r.get(1)?,
        name: r.get(2)?,
        gender: r.get(3)?,
        birth_date: r.get(4)?,
        birth_raw: r.get(5)?,
        class_label: grade.map(|g| label::class_label(g, class_name.as_deref())),
        grade,
        class_name,
        class_no: r.get(8)?,
        graduated_at: r.get(9)?,
        note: r.get(10)?,
    })
}

const SELECT: &str = "
    SELECT g.student_id, g.school_year, s.name, s.gender, s.birth_date, s.birth_raw,
           e.grade, e.class_name, e.class_no, g.graduated_at, g.note
      FROM graduations g
      JOIN students s ON s.id = g.student_id
      LEFT JOIN enrollments e
             ON e.student_id = g.student_id AND e.school_year = g.school_year";

/// 한 학년도 졸업생. `q` 는 이름 일부.
pub fn list(c: &Connection, school_year: i32, q: Option<&str>) -> AppResult<Vec<GradRow>> {
    let term = q.map(str::trim).filter(|s| !s.is_empty());
    let sql = format!(
        "{SELECT}
          WHERE g.school_year = ?1
            AND (?2 IS NULL OR s.name LIKE '%' || ?2 || '%')
          ORDER BY {order}",
        order = label::ORDER_BY_ROSTER,
    );
    let mut st = c.prepare(&sql)?;
    let rows = st
        .query_map(params![school_year, term], map)?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// 이 학생의 졸업 기록.
pub fn of_student(c: &Connection, student_id: i64) -> AppResult<Option<GradRow>> {
    Ok(c.query_row(
        &format!("{SELECT} WHERE g.student_id = ?1"),
        [student_id],
        map,
    )
    .optional()?)
}

/// 잘못 만든 졸업 기록을 되돌린다.
///
/// **사건은 지우지 않는다.** 졸업 기록만 없애고 `CANCEL` 을 덧붙인다 — 전출 취소와
/// 같은 방법이다. 지워 버리면 잘못 처리했다는 사실조차 남지 않는다.
///
/// 되돌린 학생은 그 학년도 명단으로 돌아온다. 다음 학년도 학적은 만들지 않는다 —
/// 어느 학년·반에 둘지는 사람이 정할 일이다.
pub fn cancel(c: &Connection, student_id: i64, today: NaiveDate) -> AppResult<i32> {
    let found = of_student(c, student_id)?
        .ok_or_else(|| AppError::not_found("졸업 기록을 찾을 수 없습니다."))?;

    c.execute("DELETE FROM graduations WHERE student_id = ?1", [student_id])?;
    student::add_event(
        c,
        student_id,
        found.school_year,
        "CANCEL",
        Some(&today.to_string()),
        found.grade.unwrap_or(6),
        found.class_name.as_deref(),
        found.class_no,
        Some("졸업 취소"),
        "MANUAL",
    )?;
    // 명단으로 돌아오므로 그 학년도 확인 필요를 다시 따진다
    student::sync_issues(c, student_id, found.school_year, today)?;
    Ok(found.school_year)
}

#[cfg(test)]
#[path = "graduation_tests.rs"]
mod graduation_tests;
