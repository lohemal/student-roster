//! 주소 분류·규칙 명령.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::error::AppResult;
use crate::job::{self, Job};
use crate::repo::{
    self,
    address::{CategoryRow, Matching, ReapplyResult, RuleRow, Status},
};
use crate::AppState;

// ---------------------------------------------------------------
// 분류
// ---------------------------------------------------------------

#[tauri::command]
pub fn address_category_list(
    state: State<'_, AppState>,
    school_year: i32,
) -> AppResult<Vec<CategoryRow>> {
    state
        .db
        .read(|c| repo::address::list_categories(c, school_year))
}

#[tauri::command]
pub fn address_category_create(state: State<'_, AppState>, name: String) -> AppResult<i64> {
    state.db.write(|c| repo::address::create_category(c, &name))
}

#[tauri::command]
pub fn address_category_rename(
    state: State<'_, AppState>,
    id: i64,
    name: String,
) -> AppResult<()> {
    state
        .db
        .write(|c| repo::address::rename_category(c, id, &name))
}

#[tauri::command]
pub fn address_category_delete(state: State<'_, AppState>, id: i64) -> AppResult<()> {
    state.db.write(|c| repo::address::delete_category(c, id))
}

// ---------------------------------------------------------------
// 규칙
// ---------------------------------------------------------------

#[tauri::command]
pub fn address_rule_list(state: State<'_, AppState>, school_year: i32) -> AppResult<Vec<RuleRow>> {
    state.db.read(|c| repo::address::list_rules(c, school_year))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleInput {
    pub kind: String,
    pub pattern: String,
    pub category_id: i64,
    pub note: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleCreated {
    pub rule_id: i64,
    /// 이 규칙으로 새로 분류될 학생 수 등
    pub matching: Matching,
}

/// 규칙을 만든다. **만들기만 하고 학생에게 적용하지는 않는다** —
/// 몇 명에게 걸리는지 보여 준 뒤 사용자가 적용을 고르게 한다.
#[tauri::command]
pub fn address_rule_create(
    state: State<'_, AppState>,
    input: RuleInput,
    school_year: i32,
) -> AppResult<RuleCreated> {
    state.db.write(|c| {
        let rule_id = repo::address::create_rule(
            c,
            &input.kind,
            &input.pattern,
            input.category_id,
            input.note.as_deref(),
        )?;
        let matching =
            repo::address::count_matching(c, &input.kind, &input.pattern, school_year)?;
        Ok(RuleCreated { rule_id, matching })
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleUpdate {
    pub id: i64,
    pub kind: String,
    pub pattern: String,
    pub category_id: i64,
    pub is_active: bool,
}

#[tauri::command]
pub fn address_rule_update(state: State<'_, AppState>, input: RuleUpdate) -> AppResult<()> {
    state.db.write(|c| {
        repo::address::update_rule(
            c,
            input.id,
            &input.kind,
            &input.pattern,
            input.category_id,
            input.is_active,
        )
    })
}

#[tauri::command]
pub fn address_rule_delete(state: State<'_, AppState>, id: i64) -> AppResult<()> {
    state.db.write(|c| repo::address::delete_rule(c, id))
}

/// 규칙이 몇 명에게 걸리는지 미리 세어 본다. 저장하지 않는다.
#[tauri::command]
pub fn address_rule_preview(
    state: State<'_, AppState>,
    kind: String,
    pattern: String,
    school_year: i32,
) -> AppResult<Matching> {
    state
        .db
        .read(|c| repo::address::count_matching(c, &kind, &pattern, school_year))
}

/// 규칙 하나를 그 학년도 학생에게 적용한다. 바뀐 학생 수를 돌려준다.
#[tauri::command]
pub fn address_rule_apply(
    state: State<'_, AppState>,
    rule_id: i64,
    school_year: i32,
) -> AppResult<i64> {
    let today = chrono::Local::now().date_naive();
    state.db.write(|c| {
        let changed = repo::address::apply_rule(c, rule_id, school_year)?;
        sync_year_issues(c, school_year, today)?;
        Ok(changed)
    })
}

// ---------------------------------------------------------------
// 학생 한 명
// ---------------------------------------------------------------

#[tauri::command]
pub fn address_status(state: State<'_, AppState>, student_id: i64) -> AppResult<Status> {
    state.db.read(|c| repo::address::status(c, student_id))
}

/// 이 학생만 직접 분류한다. `categoryId` 가 없으면 직접 지정을 풀고 다시 자동 판정한다.
#[tauri::command]
pub fn address_set_manual(
    state: State<'_, AppState>,
    student_id: i64,
    category_id: Option<i64>,
    school_year: i32,
) -> AppResult<()> {
    let today = chrono::Local::now().date_naive();
    state.db.write(|c| {
        repo::address::set_manual(c, student_id, category_id)?;
        repo::student::sync_issues(c, student_id, school_year, today)
    })
}

// ---------------------------------------------------------------
// 전체 다시 적용
// ---------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReapplyStarted {
    pub job_id: String,
}

/// 그 학년도 재학생 전체의 주소를 다시 판정한다. 딴 갈래에서 돌고 진행 상황을 알린다.
#[tauri::command]
pub fn address_reapply(
    app: AppHandle,
    state: State<'_, AppState>,
    school_year: i32,
) -> AppResult<ReapplyStarted> {
    let db = state.db.clone();
    let job = Job::new(app, job::new_id("address"));
    let job_id = job.id().to_string();

    std::thread::spawn(move || {
        let today = chrono::Local::now().date_naive();
        let outcome = db.write(|c| {
            job.progress("CLASSIFY", "주소 분류 확인", 0, 1);
            let out = repo::address::reapply(c, school_year, |done, total| {
                job.progress("CLASSIFY", "주소 분류 확인", done, total)
            })?;
            job.progress("ISSUES", "확인 필요 정리", 0, 1);
            sync_year_issues(c, school_year, today)?;
            job.progress("ISSUES", "확인 필요 정리", 1, 1);
            Ok::<ReapplyResult, crate::error::AppError>(out)
        });

        match outcome {
            Ok(result) => job.done(result),
            Err(e) => job.failed(e),
        }
    });

    Ok(ReapplyStarted { job_id })
}

/// 그 학년도 학생의 확인 필요를 다시 맞춘다.
///
/// 주소 판정이 바뀌면 ADDRESS 표시도 함께 바뀌어야 한다. 판정 규칙은
/// `sync_issues` 한 곳에만 있으므로 여기서도 그것을 부른다.
fn sync_year_issues(
    c: &rusqlite::Connection,
    school_year: i32,
    today: chrono::NaiveDate,
) -> AppResult<()> {
    let mut st =
        c.prepare("SELECT student_id FROM enrollments WHERE school_year = ?1")?;
    let ids: Vec<i64> = st
        .query_map([school_year], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    for id in ids {
        repo::student::sync_issues(c, id, school_year, today)?;
    }
    Ok(())
}
