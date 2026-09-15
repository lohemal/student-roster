//! 확인 필요 명령.

use serde::Serialize;
use tauri::State;

use crate::error::AppResult;
use crate::repo::{self, issue::IssueCount};
use crate::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueSummary {
    pub total: i64,
    pub by_kind: Vec<IssueCount>,
}

/// 종류별 열린 개수. 사이드바 뱃지와 '확인 필요' 화면이 함께 쓴다.
#[tauri::command]
pub fn issue_summary(state: State<'_, AppState>, school_year: i32) -> AppResult<IssueSummary> {
    let by_kind = state.db.read(|c| repo::issue::summary(c, school_year))?;
    Ok(IssueSummary {
        total: by_kind.iter().map(|k| k.count).sum(),
        by_kind,
    })
}

/// 그 학년도 학생 전체의 확인 필요를 다시 따진다.
///
/// 자료를 밖에서 고쳤거나 예전 자료를 들여온 뒤에 쓴다. 학생 한 명을 저장할 때와
/// 똑같은 규칙을 쓰므로 결과가 어긋나지 않는다. Phase 2 의 가져오기와 Phase 3 의
/// 주소 규칙 재적용도 이 자리를 쓴다.
#[tauri::command]
pub fn issue_recompute(state: State<'_, AppState>, school_year: i32) -> AppResult<i64> {
    let today = chrono::Local::now().date_naive();
    state.db.write(|c| {
        let ids: Vec<i64> = c
            .prepare("SELECT student_id FROM enrollments WHERE school_year = ?1")?
            .query_map([school_year], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;

        for id in &ids {
            repo::student::sync_issues(c, *id, school_year, today)?;
        }
        Ok(ids.len() as i64)
    })
}

/// 확인 필요 목록. 종류를 가리지 않고 한곳에 모아 본다.
#[tauri::command]
pub fn issue_list(
    state: State<'_, AppState>,
    filter: repo::issue::IssueFilter,
    limit: i64,
    offset: i64,
) -> AppResult<repo::issue::IssueListPage> {
    let limit = limit.clamp(1, 500);
    state
        .db
        .read(|c| repo::issue::list(c, &filter, limit, offset.max(0)))
}
