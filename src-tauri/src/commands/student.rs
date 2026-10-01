//! 학생 명령. 입력 검증과 `repo` 호출만 한다.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::domain::guardian;
use crate::error::{AppError, AppResult};
use crate::repo::{
    self,
    student::{
        ClassOption, ListFilter, PrimaryFillPlan, StudentDetail, StudentInput, StudentRow,
    },
};
use super::today;
use crate::AppState;

/// 한 번에 내려보내는 최대 줄 수. 화면이 느려지지 않는 선.
const MAX_PAGE: i64 = 500;
const DEFAULT_PAGE: i64 = 100;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListQuery {
    #[serde(flatten)]
    pub filter: ListFilter,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListResult {
    pub rows: Vec<StudentRow>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[tauri::command]
pub fn student_list(state: State<'_, AppState>, query: ListQuery) -> AppResult<ListResult> {
    let limit = query.limit.unwrap_or(DEFAULT_PAGE).clamp(1, MAX_PAGE);
    let offset = query.offset.unwrap_or(0).max(0);

    let page = state
        .db
        .read(|c| repo::student::list(c, &query.filter, limit, offset, today()))?;

    Ok(ListResult {
        rows: page.rows,
        total: page.total,
        limit,
        offset,
    })
}

#[tauri::command]
pub fn student_get(
    state: State<'_, AppState>,
    id: i64,
    school_year: i32,
) -> AppResult<StudentDetail> {
    state.db.read(|c| repo::student::detail(c, id, school_year, today()))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrimaryFillInput {
    /// 학생명단과 **같은 조건**. 화면에 보이던 인원과 바뀌는 인원이 같아야 한다.
    pub filter: ListFilter,
    /// `MOTHER` / `FATHER`
    pub from: String,
    /// false 면 세어만 보고 아무것도 바꾸지 않는다
    #[serde(default)]
    pub apply: bool,
}

/// 주보호자 연락처를 모·부 연락처로 **한꺼번에** 맞춘다.
///
/// `apply = false` 로 먼저 불러 몇 명이 어떻게 바뀌는지 보여 주고, 사람이 정한 뒤에
/// 같은 조건으로 다시 부른다. 미리보기와 실행이 같은 함수를 지난다.
#[tauri::command]
pub fn student_primary_fill(
    state: State<'_, AppState>,
    input: PrimaryFillInput,
) -> AppResult<PrimaryFillPlan> {
    let from = guardian::FillFrom::parse(&input.from)
        .ok_or_else(|| AppError::invalid("어느 연락처로 맞출지 골라 주세요."))?;
    let run = |c: &rusqlite::Connection| {
        repo::student::primary_fill(c, &input.filter, from, input.apply, today())
    };
    if input.apply {
        state.db.write(run)
    } else {
        state.db.read(run)
    }
}

#[tauri::command]
pub fn student_create(state: State<'_, AppState>, input: StudentInput) -> AppResult<i64> {
    let today = chrono::Local::now().date_naive();
    state.db.write(|c| repo::student::create(c, &input, today))
}

#[tauri::command]
pub fn student_update(
    state: State<'_, AppState>,
    id: i64,
    input: StudentInput,
) -> AppResult<()> {
    let today = chrono::Local::now().date_naive();
    state
        .db
        .write(|c| repo::student::update(c, id, &input, today))
}

/// 입력 실수를 되돌린다. 화면에서 한 번 더 확인을 받는다.
#[tauri::command]
pub fn student_delete(state: State<'_, AppState>, id: i64, confirm: bool) -> AppResult<()> {
    if !confirm {
        return Err(AppError::invalid("삭제를 확인하지 않았습니다."));
    }
    state.db.write(|c| repo::student::delete(c, id))
}

/// 검색 칸의 선택지 — 그 학년도에 실제로 있는 학년·반.
#[tauri::command]
pub fn student_class_options(
    state: State<'_, AppState>,
    school_year: i32,
) -> AppResult<Vec<ClassOption>> {
    state.db.read(|c| repo::student::class_options(c, school_year, today()))
}
