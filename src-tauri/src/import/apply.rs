//! 분석 결과를 실제로 저장한다.
//!
//! 지키는 것
//!   * **전부 아니면 전무.** 한 트랜잭션 안에서 끝낸다 — 1,200명 중 500명만 들어가고
//!     나머지가 실패하는 상태를 만들지 않는다.
//!   * 저장 규칙은 `repo::student` 것을 그대로 쓴다. 가져오기용으로 다시 만들지 않는다.
//!   * 판정을 **트랜잭션 안에서 다시** 돌린다. 미리보기와 결과가 어긋나지 않는다.

use chrono::NaiveDate;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use super::analyze::{self, Action};
use super::row::{self, ParsedRow};
use crate::error::AppResult;
use crate::repo::student;

/// 무엇을 적용할지. 화면에서 사용자가 고른다.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Choice {
    /// 새 학생을 등록한다
    #[serde(default = "yes")]
    pub add: bool,
    /// 확실한 기존 학생을 갱신한다
    #[serde(default = "yes")]
    pub update: bool,
    /// 이 엑셀 줄 번호는 건너뛴다
    #[serde(default)]
    pub skip_rows: Vec<usize>,
}

fn yes() -> bool {
    true
}

impl Default for Choice {
    fn default() -> Self {
        Self {
            add: true,
            update: true,
            skip_rows: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyResult {
    pub total: usize,
    pub added: usize,
    pub updated: usize,
    pub unchanged: usize,
    /// 중복 의심·가져올 수 없음·사용자가 뺀 줄
    pub skipped: usize,
    /// 그 학년도 학생에게 열려 있는 확인 필요 건수 (저장 뒤 실제 값)
    pub issue_count: i64,
    /// 이번 가져오기 뒤에 새로 찾은 형제 후보 쌍
    pub sibling_candidates: i64,
    pub import_id: i64,
}

#[derive(Debug, Clone)]
pub struct Meta {
    pub file_name: String,
    pub sheet_name: String,
    pub school_year: i32,
    pub mapping_json: String,
}

/// 저장한다. `on_progress(stage, label, done, total)`.
pub fn run(
    c: &Connection,
    rows: &[ParsedRow],
    meta: &Meta,
    choice: &Choice,
    today: NaiveDate,
    mut on_progress: impl FnMut(&str, &str, usize, usize),
) -> AppResult<ApplyResult> {
    let year = meta.school_year;
    let total = rows.len();

    // 1) 판정을 다시 한다 — 미리보기 이후 자료가 달라졌더라도 어긋나지 않도록
    on_progress("ANALYZE", "자료 분석", 0, total);
    let analyzed = analyze::run(c, rows, year, |done, t| {
        on_progress("ANALYZE", "자료 분석", done, t)
    })?;

    // 2) 저장
    let mut out = ApplyResult {
        total,
        ..Default::default()
    };
    let step = analyze::progress_step(total);
    on_progress("SAVE", "학생정보 저장", 0, total);

    for (i, (plan, row)) in analyzed.rows.iter().zip(rows.iter()).enumerate() {
        let skipped_by_user = choice.skip_rows.contains(&plan.excel_row);

        match plan.action {
            Action::Add if choice.add && !skipped_by_user => {
                student::create(c, &row::to_input(row, year), today)?;
                out.added += 1;
            }
            Action::Update if choice.update && !skipped_by_user => {
                let id = plan.student_id.expect("갱신 대상에는 학생 번호가 있다");
                let existing = analyze::load_existing(c, id, year)?;
                let (input, _) = analyze::merge(&existing, row, year);
                // 지난 학년도에만 있던 학생이면 이 학년도 학적을 먼저 만든다
                if !existing.has_enrollment {
                    student::ensure_enrollment(
                        c,
                        id,
                        year,
                        input.grade,
                        input.class_name.as_deref(),
                        input.class_no,
                        "IMPORT",
                    )?;
                }
                student::update(c, id, &input, today)?;
                out.updated += 1;
            }
            Action::Unchanged if !skipped_by_user => out.unchanged += 1,
            _ => out.skipped += 1,
        }

        if i % step == 0 {
            on_progress("SAVE", "학생정보 저장", i, total);
        }
    }
    on_progress("SAVE", "학생정보 저장", total, total);

    // 3) 형제 후보 — 줄마다 찾으면 같은 일을 1,200번 하게 되므로 다 넣은 뒤 한 번만 한다
    on_progress("SIBLING", "형제 후보 찾기", 0, 1);
    let sib = crate::repo::sibling::scan(c, year, today, |_, _, _| {})?;
    out.sibling_candidates = sib.new_candidates;

    // 관계가 걸린 학생만 표시를 다시 맞춘다 (전체를 다시 훑을 필요는 없다)
    let linked: Vec<i64> = c
        .prepare(
            "SELECT DISTINCT student_id FROM (
                 SELECT student_a AS student_id FROM sibling_links
                 UNION SELECT student_b FROM sibling_links)",
        )?
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    for id in linked {
        student::sync_issues(c, id, year, today)?;
    }
    on_progress("SIBLING", "형제 후보 찾기", 1, 1);

    // 4) 마무리 — 기록을 남기고 실제 확인 필요 건수를 센다
    on_progress("FINISH", "마무리", 0, 1);
    out.issue_count = c.query_row(
        "SELECT COUNT(*) FROM issues i
           JOIN enrollments e ON e.student_id = i.student_id AND e.school_year = ?1
          WHERE i.status = 'OPEN'",
        [year],
        |r| r.get(0),
    )?;

    let mode = match (choice.add, choice.update) {
        (true, true) => "BOTH",
        (true, false) => "ADD",
        (false, true) => "UPDATE",
        (false, false) => "ADD",
    };
    let summary = serde_json::json!({
        "add": analyzed.summary.add,
        "update": analyzed.summary.update,
        "unchanged": analyzed.summary.unchanged,
        "ambiguous": analyzed.summary.ambiguous,
        "blocked": analyzed.summary.blocked,
    })
    .to_string();

    c.execute(
        "INSERT INTO imports(
            file_name, sheet_name, school_year, mode,
            total, added, updated, skipped, flagged, mapping, summary)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
        rusqlite::params![
            meta.file_name,
            meta.sheet_name,
            year,
            mode,
            out.total as i64,
            out.added as i64,
            out.updated as i64,
            out.skipped as i64,
            analyzed.summary.warned as i64,
            meta.mapping_json,
            summary,
        ],
    )?;
    out.import_id = c.last_insert_rowid();
    on_progress("FINISH", "마무리", 1, 1);

    Ok(out)
}

#[cfg(test)]
#[path = "apply_tests.rs"]
mod apply_tests;
