//! 전입·전출 명령.
//!
//! 규칙은 `repo::transfer` 에 있다. 여기서는 받아 넘기고 오늘 날짜만 붙인다.

use tauri::State;

use crate::error::AppResult;
use crate::repo::{
    self,
    stats::GradeCounts,
    transfer::{
        CancelInResult, MoveRow, PastOutInput, StudentMatch, TransferInInput, TransferInResult,
        TransferOutInput,
    },
};
use super::today;
use crate::AppState;

/// 이름·생년월일로 기존 학생을 찾는다. **같은 학생인지는 사람이 정한다.**
#[tauri::command]
pub fn transfer_search_students(
    state: State<'_, AppState>,
    name: String,
    birth: Option<String>,
    school_year: i32,
) -> AppResult<Vec<StudentMatch>> {
    state
        .db
        .read(|c| repo::transfer::search_students(c, &name, birth.as_deref(), school_year))
}

/// 그 학년의 반별 남/여/미입력 인원. **반을 골라 주지는 않는다.**
#[tauri::command]
pub fn transfer_class_counts(
    state: State<'_, AppState>,
    school_year: i32,
    grade: i32,
) -> AppResult<GradeCounts> {
    state.db.read(|c| repo::stats::grade_counts(c, school_year, grade, today()))
}

/// 그 반에서 이미 쓰이는 번호. 저장하기 전에 겹침을 알려 주는 데 쓴다.
#[tauri::command]
pub fn transfer_used_numbers(
    state: State<'_, AppState>,
    school_year: i32,
    grade: i32,
    class_name: Option<String>,
) -> AppResult<Vec<i32>> {
    state
        .db
        .read(|c| repo::stats::used_numbers(c, school_year, grade, class_name.as_deref(), today()))
}

/// 그 학년도의 전입생 또는 전출생 목록.
#[tauri::command]
pub fn transfer_list(
    state: State<'_, AppState>,
    school_year: i32,
    want_in: bool,
    grade: Option<i32>,
    q: Option<String>,
) -> AppResult<Vec<MoveRow>> {
    state
        .db
        .read(|c| repo::transfer::list(c, school_year, want_in, grade, q.as_deref(), today()))
}

/// 전입 처리.
#[tauri::command]
pub fn transfer_in(
    state: State<'_, AppState>,
    input: TransferInInput,
) -> AppResult<TransferInResult> {
    state
        .db
        .write(|c| repo::transfer::transfer_in(c, &input, today()))
}

/// 전출 처리. 학생도 학적도 지우지 않는다.
#[tauri::command]
pub fn transfer_out(state: State<'_, AppState>, input: TransferOutInput) -> AppResult<()> {
    state
        .db
        .write(|c| repo::transfer::transfer_out(c, &input, today()))
}

/// 전출 처리 취소. 사건은 남기고 상태만 되돌린다.
#[tauri::command]
pub fn transfer_out_cancel(
    state: State<'_, AppState>,
    student_id: i64,
    school_year: i32,
) -> AppResult<()> {
    state
        .db
        .write(|c| repo::transfer::transfer_out_cancel(c, student_id, school_year, today()))
}

/// 아직 오지 않은 전입·전출의 **예정일만** 바꾼다.
///
/// 상태를 손으로 고치는 길이 아니다 — 날짜만 바꾸면 현재 재학생 판정은 저절로 따라온다.
#[tauri::command]
pub fn transfer_reschedule(
    state: State<'_, AppState>,
    student_id: i64,
    school_year: i32,
    date: String,
) -> AppResult<String> {
    state
        .db
        .write(|c| repo::transfer::reschedule(c, student_id, school_year, &date, today()))
}

/// 전입 예정을 되돌린다. 이번에 처음 만든 학생이면 학생 자료까지 지운다.
#[tauri::command]
pub fn transfer_in_cancel(
    state: State<'_, AppState>,
    student_id: i64,
    school_year: i32,
) -> AppResult<CancelInResult> {
    state
        .db
        .write(|c| repo::transfer::transfer_in_cancel(c, student_id, school_year, today()))
}

/// 프로그램을 쓰기 전에 이미 나간 학생을 뒤늦게 적어 넣는다.
#[tauri::command]
pub fn transfer_past_out(state: State<'_, AppState>, input: PastOutInput) -> AppResult<i64> {
    state
        .db
        .write(|c| repo::transfer::add_past_transfer_out(c, &input, today()))
}
