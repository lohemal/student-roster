//! 내보낼 학생을 읽어 온다.
//!
//! **학생명단과 같은 조건, 같은 차례로 읽는다.** 명단에서 48명이 보였는데 파일에
//! 47명이 들어 있으면 둘 다 못 쓴다. 그래서 `ListFilter` 와 `ORDER_BY_ROSTER` 를
//! 그대로 쓰고 내보내기 전용 조건이나 정렬을 따로 만들지 않는다.

use chrono::NaiveDate;
use rusqlite::{params_from_iter, Connection};

use crate::domain::label;
use crate::error::AppResult;
use crate::repo::sibling as sibling_repo;
use crate::repo::student::{build_where_pub, ListFilter};

/// 내보내기에 필요한 학생 한 명. 어떤 열을 고르든 이 하나로 만든다.
#[derive(Debug, Clone)]
pub struct Row {
    pub student_id: i64,
    pub school_year: i32,
    pub grade: i32,
    pub class_name: Option<String>,
    pub class_no: Option<i32>,
    pub name: String,
    pub gender: Option<String>,
    pub birth_date: Option<String>,
    pub birth_raw: Option<String>,
    pub address_raw: Option<String>,
    pub address_category: Option<String>,
    pub father_name: Option<String>,
    pub mother_name: Option<String>,
    pub father_phone: Option<String>,
    pub mother_phone: Option<String>,
    pub primary_phone: Option<String>,
    pub note: Option<String>,
    /// 지금 함께 다니는 확정 형제의 이름표. 저장된 값이 아니라 볼 때마다 만든다.
    pub siblings: Vec<String>,
}

/// 조건에 드는 학생을 학생명단과 같은 차례로 읽는다.
pub fn rows(c: &Connection, f: &ListFilter, asof: NaiveDate) -> AppResult<Vec<Row>> {
    let w = build_where_pub(f, asof);
    let where_sql = w.0.join(" AND ");

    let sql = format!(
        "SELECT e.student_id, e.school_year, e.grade, e.class_name, e.class_no,
                s.name, s.gender, s.birth_date, s.birth_raw,
                s.address_raw, ac.name,
                s.father_name, s.mother_name,
                s.father_phone, s.mother_phone, s.primary_phone, s.note
           FROM enrollments e
           JOIN students s ON s.id = e.student_id
           LEFT JOIN address_categories ac ON ac.id = s.address_category_id
          WHERE {where_sql}
          ORDER BY {order}",
        order = label::ORDER_BY_ROSTER,
    );

    let mut st = c.prepare(&sql)?;
    let mut out: Vec<Row> = st
        .query_map(params_from_iter(w.1.iter()), |r| {
            Ok(Row {
                student_id: r.get(0)?,
                school_year: r.get(1)?,
                grade: r.get(2)?,
                class_name: r.get(3)?,
                class_no: r.get(4)?,
                name: r.get(5)?,
                gender: r.get(6)?,
                birth_date: r.get(7)?,
                birth_raw: r.get(8)?,
                address_raw: r.get(9)?,
                address_category: r.get(10)?,
                father_name: r.get(11)?,
                mother_name: r.get(12)?,
                father_phone: r.get(13)?,
                mother_phone: r.get(14)?,
                primary_phone: r.get(15)?,
                note: r.get(16)?,
                siblings: Vec::new(),
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    drop(st);

    // 형제 이름표는 저장해 두지 않는다 — 진급하면 바뀌어야 하므로 여기서 만든다
    for row in out.iter_mut() {
        row.siblings = sibling_repo::labels_of(c, row.student_id, f.school_year, asof)?;
    }
    Ok(out)
}

#[cfg(test)]
#[path = "export_tests.rs"]
mod export_tests;
