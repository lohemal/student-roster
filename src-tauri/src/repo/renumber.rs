//! 학생 번호 재정렬 — 읽고, 미리 보여 주고, 저장한다.
//!
//! 흐름은 Phase 2 가져오기와 같다. **계산(`preview`) → 사람이 확인 → 저장(`apply`)**.
//!
//! 지키는 것
//!   * 계산은 `domain::renumber` 가 한다. 여기서 번호를 따로 셈하지 않는다.
//!   * `apply` 는 계획을 받아 그대로 쓰지 않는다. **저장 직전에 다시 계산한다.**
//!     미리보기를 띄워 둔 사이에 명단이 바뀌었을 수 있기 때문이다.
//!   * 전부 아니면 전무. `Db::write` 트랜잭션 안에서 끝낸다.

use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::domain::enroll::ACTIVE_STATUS_SQL as ACTIVE;
use crate::domain::label;
use crate::domain::renumber::{self, Kind, Plan, Seat};
use crate::error::{AppError, AppResult};
use crate::repo::student;

/// 한 반의 자리 하나 — 화면에 이름까지 보여 주기 위해 도메인의 `Seat` 에 이름을 붙였다.
#[derive(Debug, Clone)]
struct Row {
    student_id: i64,
    name: String,
    class_no: Option<i32>,
}

/// 그 학생이 속한 반.
#[derive(Debug, Clone)]
pub struct ClassRef {
    pub grade: i32,
    pub class_name: Option<String>,
}

impl ClassRef {
    pub fn label(&self) -> String {
        label::class_label(self.grade, self.class_name.as_deref())
    }
}

fn class_of(c: &Connection, student_id: i64, school_year: i32) -> AppResult<ClassRef> {
    let found: Option<(i32, Option<String>)> = c
        .query_row(
            "SELECT grade, class_name FROM enrollments
              WHERE student_id = ?1 AND school_year = ?2",
            params![student_id, school_year],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let (grade, class_name) =
        found.ok_or_else(|| AppError::not_found("이 학년도 학적을 찾을 수 없습니다."))?;
    Ok(ClassRef { grade, class_name })
}

/// 같은 학년도·학년·반 학생 전부. 번호가 없는 학생도 담는다.
///
/// 반 이름이 NULL 인 학생(반배정 미정)끼리도 하나의 묶음으로 본다.
fn rows_of(c: &Connection, school_year: i32, class: &ClassRef) -> AppResult<Vec<Row>> {
    let mut st = c.prepare(&format!(
        "SELECT e.student_id, s.name, e.class_no
           FROM enrollments e
           JOIN students s ON s.id = e.student_id
          WHERE e.school_year = ?1
            AND e.grade = ?2
            AND ((e.class_name IS NULL AND ?3 IS NULL) OR e.class_name = ?3)
            AND e.{ACTIVE}
          ORDER BY CASE WHEN e.class_no IS NULL THEN 1 ELSE 0 END, e.class_no, s.name",
    ))?;
    let rows = st
        .query_map(params![school_year, class.grade, class.class_name], |r| {
            Ok(Row {
                student_id: r.get(0)?,
                name: r.get(1)?,
                class_no: r.get(2)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

fn seats_of(rows: &[Row]) -> Vec<Seat> {
    rows.iter()
        .map(|r| Seat::new(r.student_id, r.class_no))
        .collect()
}

// ---------------------------------------------------------------
// 미리보기
// ---------------------------------------------------------------

/// 미리보기 표의 한 줄.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewRow {
    pub student_id: i64,
    pub name: String,
    pub from: Option<i32>,
    pub to: i32,
    /// 사용자가 직접 고른 학생인지
    pub is_target: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preview {
    pub student_id: i64,
    pub name: String,
    pub class_label: String,
    pub from: Option<i32>,
    pub to: i32,
    /// 무엇을 하는 계획인지 — NONE / ASSIGN / REORDER / INSERT
    pub kind: String,
    /// 사용자에게 보여 줄 한 줄 설명
    pub summary: String,
    /// 번호가 바뀌는 학생만. 대상 학생도 들어 있다.
    pub rows: Vec<PreviewRow>,
    /// 대상을 뺀, 덩달아 바뀌는 인원
    pub affected: i64,
    /// 저장할 때 명단이 그대로인지 확인하는 값
    pub state_key: String,
    /// 할 수 없는 경우 그 까닭. 있으면 [변경 적용]을 막는다.
    pub blocked: Option<String>,
}

/// 번호를 바꾸면 무슨 일이 생기는지 계산한다. **DB 는 건드리지 않는다.**
pub fn preview(
    c: &Connection,
    student_id: i64,
    school_year: i32,
    new_no: i32,
) -> AppResult<Preview> {
    let class = class_of(c, student_id, school_year)?;
    let rows = rows_of(c, school_year, &class)?;
    let seats = seats_of(&rows);
    let class_label = class.label();

    let me = rows
        .iter()
        .find(|r| r.student_id == student_id)
        .ok_or_else(|| AppError::not_found("학생을 찾을 수 없습니다."))?;

    let base = Preview {
        student_id,
        name: me.name.clone(),
        class_label: class_label.clone(),
        from: me.class_no,
        to: new_no,
        kind: Kind::None.code().to_string(),
        summary: String::new(),
        rows: Vec::new(),
        affected: 0,
        state_key: renumber::state_key(&seats),
        blocked: None,
    };

    let plan = match renumber::plan(&seats, student_id, new_no) {
        Ok(p) => p,
        Err(why) => {
            return Ok(Preview {
                blocked: Some(why.message(&class_label)),
                summary: "번호를 바꿀 수 없습니다.".to_string(),
                ..base
            })
        }
    };

    let name_of = |id: i64| {
        rows.iter()
            .find(|r| r.student_id == id)
            .map(|r| r.name.clone())
            .unwrap_or_default()
    };

    let mut out: Vec<PreviewRow> = plan
        .moves
        .iter()
        .map(|m| PreviewRow {
            student_id: m.student_id,
            name: name_of(m.student_id),
            from: m.from,
            to: m.to,
            is_target: m.student_id == student_id,
        })
        .collect();
    // 바뀐 뒤의 번호 순서로 보여 준다 — 적용 후 명단과 같은 차례가 된다
    out.sort_by_key(|r| r.to);

    let affected = plan.affected(student_id) as i64;
    Ok(Preview {
        kind: plan.kind.code().to_string(),
        summary: summary_of(&plan, &class_label, me.class_no, new_no, affected),
        rows: out,
        affected,
        ..base
    })
}

fn summary_of(
    plan: &Plan,
    class_label: &str,
    from: Option<i32>,
    to: i32,
    affected: i64,
) -> String {
    match plan.kind {
        Kind::None => "이미 그 번호입니다. 바뀌는 것이 없습니다.".to_string(),
        Kind::Assign => match from {
            Some(f) => format!("{f}번에서 {to}번으로 바꿉니다. {to}번은 비어 있어 다른 학생은 바뀌지 않습니다."),
            None => format!("{to}번을 줍니다. {to}번은 비어 있어 다른 학생은 바뀌지 않습니다."),
        },
        Kind::Reorder => format!(
            "순서를 지키며 {to}번으로 옮깁니다. 같은 {class_label}반 학생 {affected}명의 번호가 함께 바뀝니다."
        ),
        Kind::Insert => format!(
            "{to}번 자리에 끼워 넣습니다. 같은 {class_label}반 학생 {affected}명의 번호가 한 칸씩 밀립니다."
        ),
    }
}

// ---------------------------------------------------------------
// 적용
// ---------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyResult {
    /// 번호가 바뀐 학생 수 (대상 포함)
    pub moved: i64,
    pub class_label: String,
    pub op_id: Option<i64>,
}

/// 계획을 실제로 저장한다.
///
/// `state_key` 는 미리보기를 만들 때 받은 값이다. 그 사이에 명단이 달라졌으면
/// 저장하지 않고 다시 보라고 알린다 — 오래된 미리보기로 엉뚱한 학생의 번호를
/// 바꾸는 일을 막는다.
pub fn apply(
    c: &Connection,
    student_id: i64,
    school_year: i32,
    new_no: i32,
    state_key: &str,
    today: NaiveDate,
) -> AppResult<ApplyResult> {
    let class = class_of(c, student_id, school_year)?;
    let rows = rows_of(c, school_year, &class)?;
    let seats = seats_of(&rows);
    let class_label = class.label();

    // 미리보기 이후 명단이 바뀌었는지 — 번호·인원 어느 쪽이 달라져도 걸린다
    if renumber::state_key(&seats) != state_key {
        return Err(AppError::new(
            "STALE",
            "그 사이 명단이 바뀌었습니다. 번호 변경을 다시 확인해 주세요.",
        ));
    }

    // 저장 직전에 다시 계산한다. 미리보기와 같은 자료에서 같은 규칙으로 나온 값이다.
    let plan = renumber::plan(&seats, student_id, new_no)
        .map_err(|why| AppError::invalid(why.message(&class_label)))?;

    if plan.moves.is_empty() {
        return Ok(ApplyResult {
            moved: 0,
            class_label,
            op_id: None,
        });
    }

    for m in &plan.moves {
        c.execute(
            "UPDATE enrollments
                SET class_no = ?3, updated_at = datetime('now','localtime')
              WHERE student_id = ?1 AND school_year = ?2",
            params![m.student_id, school_year, m.to],
        )?;
    }

    // 나중에 '왜 번호가 바뀌었는지' 를 되짚을 수 있도록 작업 한 줄만 남긴다
    let me = rows.iter().find(|r| r.student_id == student_id);
    c.execute(
        "INSERT INTO renumber_ops(
            school_year, grade, class_name, student_id,
            from_no, to_no, kind, moved, plan)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![
            school_year,
            class.grade,
            class.class_name,
            student_id,
            me.and_then(|r| r.class_no),
            new_no,
            plan.kind.code(),
            plan.moves.len() as i64,
            serde_json::to_string(&plan.moves).unwrap_or_else(|_| "[]".into()),
        ],
    )?;
    let op_id = c.last_insert_rowid();

    // 표시를 다시 맞춘다. **그 반 학생 전부**를 본다 — 번호가 그대로인 학생도
    // 짝이 비켜 주면 '번호 중복' 이 풀리기 때문이다. 한 반은 수십 명이라 가볍고,
    // 주소나 형제까지 다시 훑지는 않는다.
    for row in &rows {
        student::sync_issues(c, row.student_id, school_year, today)?;
    }

    Ok(ApplyResult {
        moved: plan.moves.len() as i64,
        class_label,
        op_id: Some(op_id),
    })
}

#[cfg(test)]
#[path = "renumber_tests.rs"]
mod renumber_tests;
