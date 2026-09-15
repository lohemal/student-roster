//! 학생 번호 재정렬 명령.
//!
//! **계산과 저장을 나눈다.** 화면은 먼저 `renumber_preview` 로 무슨 일이 생기는지
//! 받아 사용자에게 보여 주고, 사용자가 [변경 적용]을 누를 때 `renumber_apply` 를
//! 부른다. 번호는 화면에서 셈하지 않는다.

use serde::Deserialize;
use tauri::State;

use crate::error::AppResult;
use crate::repo::{
    self,
    renumber::{ApplyResult, Preview},
};
use crate::AppState;

/// 번호를 바꾸면 같은 반에 무슨 일이 생기는지 미리 계산한다. DB 는 건드리지 않는다.
#[tauri::command]
pub fn renumber_preview(
    state: State<'_, AppState>,
    student_id: i64,
    school_year: i32,
    new_no: i32,
) -> AppResult<Preview> {
    state
        .db
        .read(|c| repo::renumber::preview(c, student_id, school_year, new_no))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyInput {
    pub student_id: i64,
    pub school_year: i32,
    pub new_no: i32,
    /// 미리보기를 받을 때 함께 온 값. 그 사이 명단이 바뀌었는지 가린다.
    pub state_key: String,
}

/// 미리 보여 준 계획을 저장한다. 한 트랜잭션 안에서 끝난다.
#[tauri::command]
pub fn renumber_apply(state: State<'_, AppState>, input: ApplyInput) -> AppResult<ApplyResult> {
    let today = chrono::Local::now().date_naive();
    state.db.write(|c| {
        repo::renumber::apply(
            c,
            input.student_id,
            input.school_year,
            input.new_no,
            &input.state_key,
            today,
        )
    })
}
