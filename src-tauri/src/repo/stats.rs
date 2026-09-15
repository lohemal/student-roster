//! 인원 집계.
//!
//! 전입 반 배정을 도우려고 만들었지만 **화면 전용이 아니다.** Phase 7 통계가 같은
//! 함수를 쓴다. 숫자를 세는 규칙이 두 군데 있으면 언젠가 서로 다른 값을 말한다.
//!
//! 지키는 것
//!   * 세는 대상은 언제나 **현재 재학생**(`domain::enroll::ACTIVE_STATUS_SQL`).
//!     전출한 학생은 그 즉시 빠진다.
//!   * 성별이 비어 있는 학생을 남·여 어느 쪽에도 **넣지 않는다.** 따로 센다.
//!     남 + 여 가 합계와 다른데 까닭을 알 수 없는 표를 만들지 않기 위해서다.

use rusqlite::Connection;
use serde::Serialize;

use crate::domain::enroll::ACTIVE_STATUS_SQL as ACTIVE;
use crate::domain::label;
use crate::error::AppResult;

/// 한 반의 인원.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClassCount {
    pub grade: i32,
    pub class_name: Option<String>,
    /// `3-나리` / 반이 없으면 `3-미정`
    pub class_label: String,
    pub male: i64,
    pub female: i64,
    /// 성별이 비어 있거나 알 수 없는 학생
    pub unknown: i64,
    pub total: i64,
}

impl ClassCount {
    fn empty(grade: i32, class_name: Option<String>) -> Self {
        Self {
            class_label: label::class_label(grade, class_name.as_deref()),
            grade,
            class_name,
            male: 0,
            female: 0,
            unknown: 0,
            total: 0,
        }
    }
}

/// 학년·반별 남/여/미입력 인원.
///
/// `grade` 를 주면 그 학년만, 주지 않으면 전 학년을 센다.
/// 정렬은 학생명단과 같다 — 숫자 반이 먼저, 그다음 글자 반, 반 없는 학생은 맨 뒤.
pub fn class_counts(
    c: &Connection,
    school_year: i32,
    grade: Option<i32>,
) -> AppResult<Vec<ClassCount>> {
    let sql = format!(
        "SELECT e.grade, e.class_name,
                SUM(CASE WHEN s.gender = 'M' THEN 1 ELSE 0 END),
                SUM(CASE WHEN s.gender = 'F' THEN 1 ELSE 0 END),
                SUM(CASE WHEN s.gender IS NULL OR s.gender NOT IN ('M','F') THEN 1 ELSE 0 END),
                COUNT(*)
           FROM enrollments e
           JOIN students s ON s.id = e.student_id
          WHERE e.school_year = ?1
            AND (?2 IS NULL OR e.grade = ?2)
            AND e.{ACTIVE}
          GROUP BY e.grade, e.class_name
          ORDER BY e.grade,
                   CASE WHEN e.class_name IS NULL OR TRIM(e.class_name) = '' THEN 2
                        WHEN TRIM(e.class_name) GLOB '[0-9]*' THEN 0
                        ELSE 1 END,
                   CASE WHEN TRIM(e.class_name) GLOB '[0-9]*'
                        THEN CAST(e.class_name AS INTEGER) ELSE 0 END,
                   e.class_name"
    );
    let mut st = c.prepare(&sql)?;
    let rows = st
        .query_map(rusqlite::params![school_year, grade], |r| {
            let grade: i32 = r.get(0)?;
            let class_name: Option<String> = r.get(1)?;
            Ok(ClassCount {
                class_label: label::class_label(grade, class_name.as_deref()),
                grade,
                class_name,
                male: r.get(2)?,
                female: r.get(3)?,
                unknown: r.get(4)?,
                total: r.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// 한 학년의 반별 인원 + 그 학년 합계.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GradeCounts {
    pub grade: i32,
    pub classes: Vec<ClassCount>,
    pub total: ClassCount,
}

/// 전입 반 배정 화면이 쓰는 모양. 그 학년의 반별 인원과 합계를 함께 준다.
///
/// 아직 학생이 한 명도 없는 반은 나오지 않는다. 새 반을 만들고 싶으면 반 이름을
/// 직접 적으면 된다 — 프로그램이 반 목록을 정해 주지 않는다.
pub fn grade_counts(c: &Connection, school_year: i32, grade: i32) -> AppResult<GradeCounts> {
    let classes = class_counts(c, school_year, Some(grade))?;
    let mut total = ClassCount::empty(grade, None);
    total.class_label = format!("{grade}학년 합계");
    for r in &classes {
        total.male += r.male;
        total.female += r.female;
        total.unknown += r.unknown;
        total.total += r.total;
    }
    Ok(GradeCounts {
        grade,
        classes,
        total,
    })
}

/// 그 반에서 이미 쓰이고 있는 번호.
///
/// 전입 학생에게 줄 번호가 겹치는지 **저장하기 전에** 알려 주는 데 쓴다.
/// 겹친다고 막지는 않는다 — 겹친 채 저장되면 Phase 5 의 '번호 중복' 표시가 켜진다.
pub fn used_numbers(
    c: &Connection,
    school_year: i32,
    grade: i32,
    class_name: Option<&str>,
) -> AppResult<Vec<i32>> {
    let sql = format!(
        "SELECT DISTINCT class_no FROM enrollments
          WHERE school_year = ?1 AND grade = ?2
            AND ((class_name IS NULL AND ?3 IS NULL) OR class_name = ?3)
            AND class_no IS NOT NULL
            AND {ACTIVE}
          ORDER BY class_no"
    );
    let mut st = c.prepare(&sql)?;
    let rows = st
        .query_map(rusqlite::params![school_year, grade, class_name], |r| {
            r.get(0)
        })?
        .collect::<rusqlite::Result<Vec<i32>>>()?;
    Ok(rows)
}

#[cfg(test)]
#[path = "stats_tests.rs"]
mod stats_tests;
