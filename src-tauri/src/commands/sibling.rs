//! 본교 형제 명령.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::domain::sibling::Field;
use crate::error::{AppError, AppResult};
use crate::job::{self, Job};
use crate::repo::{
    self,
    sibling::{BatchConfirm, CandidateRow, ScanResult, SiblingView},
};
use super::today;
use crate::AppState;

/// 한 학생의 형제 관계 (확정 · 후보 · 형제 아님).
#[tauri::command]
pub fn sibling_list(
    state: State<'_, AppState>,
    student_id: i64,
    school_year: i32,
) -> AppResult<Vec<SiblingView>> {
    state
        .db
        .read(|c| repo::sibling::list_for_student(c, student_id, school_year, today()))
}

/// 형제로 확인한다.
#[tauri::command]
pub fn sibling_confirm(
    state: State<'_, AppState>,
    link_id: i64,
    school_year: i32,
) -> AppResult<()> {
    decide(state, link_id, school_year, Decision::Confirm)
}

/// 형제가 아니라고 정한다. 다시 훑어도 후보로 되살아나지 않는다.
#[tauri::command]
pub fn sibling_reject(
    state: State<'_, AppState>,
    link_id: i64,
    school_year: i32,
) -> AppResult<()> {
    decide(state, link_id, school_year, Decision::Reject)
}

/// 결정을 물리고 다시 후보로 둔다.
#[tauri::command]
pub fn sibling_reset(state: State<'_, AppState>, link_id: i64, school_year: i32) -> AppResult<()> {
    decide(state, link_id, school_year, Decision::Reset)
}

enum Decision {
    Confirm,
    Reject,
    Reset,
}

fn decide(
    state: State<'_, AppState>,
    link_id: i64,
    school_year: i32,
    what: Decision,
) -> AppResult<()> {
    let today = chrono::Local::now().date_naive();
    state.db.write(|c| {
        let (a, b) = match what {
            Decision::Confirm => repo::sibling::confirm(c, link_id)?,
            Decision::Reject => repo::sibling::reject(c, link_id)?,
            Decision::Reset => repo::sibling::reset(c, link_id)?,
        };
        // 두 학생 모두 표시를 다시 맞춘다
        repo::student::sync_issues(c, a, school_year, today)?;
        repo::student::sync_issues(c, b, school_year, today)?;
        Ok(())
    })
}

/// 확인 필요 화면에 떠 있는 형제 후보를 쌍마다 한 줄로.
#[tauri::command]
pub fn sibling_candidates(
    state: State<'_, AppState>,
    school_year: i32,
) -> AppResult<Vec<CandidateRow>> {
    state.db.read(|c| repo::sibling::candidates(c, school_year, today()))
}

/// 고른 형제 후보를 **한 번에** 확정한다.
///
/// 후보마다 명령을 부르지 않는다. 쉰 건을 확정하다 서른 건째에서 멈추면 절반만
/// 형제가 되고 화면은 무엇이 끝났는지 알 수 없다. 여기서는 한 트랜잭션 안에서
/// 확정하고, 걸린 학생의 확인 필요까지 다시 맞춘 뒤에 끝난다.
#[tauri::command]
pub fn sibling_confirm_many(
    state: State<'_, AppState>,
    link_ids: Vec<i64>,
    school_year: i32,
) -> AppResult<BatchConfirm> {
    if link_ids.is_empty() {
        return Err(AppError::invalid("확정할 형제 후보를 골라 주세요."));
    }
    let today = chrono::Local::now().date_naive();
    state.db.write(|c| {
        let out = repo::sibling::confirm_many(c, &link_ids)?;
        // 보호자 정보 보완·불일치 판정은 sync_issues 한 곳에만 있다.
        // 일괄 확정이라고 건너뛰면 형제로 확정한 뒤에야 보이는 표시가 생기지 않는다.
        for id in &out.students {
            repo::student::sync_issues(c, *id, school_year, today)?;
        }
        Ok(out)
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FillInput {
    pub link_id: i64,
    /// 채울 학생 (형제 둘 가운데 하나)
    pub student_id: i64,
    /// 가져올 항목 — `fatherName` 처럼 보낸다
    pub fields: Vec<String>,
    pub school_year: i32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FillResult {
    /// 실제로 채워진 항목 이름
    pub filled: Vec<String>,
}

/// 형제에게 있는 보호자 정보를 **빈칸에만** 가져온다.
#[tauri::command]
pub fn sibling_fill(state: State<'_, AppState>, input: FillInput) -> AppResult<FillResult> {
    let fields: Vec<Field> = input
        .fields
        .iter()
        .map(|f| {
            Field::parse(f).ok_or_else(|| AppError::invalid("가져올 항목이 올바르지 않습니다."))
        })
        .collect::<AppResult<_>>()?;
    if fields.is_empty() {
        return Err(AppError::invalid("가져올 항목을 골라 주세요."));
    }

    let today = chrono::Local::now().date_naive();
    state.db.write(|c| {
        let filled =
            repo::sibling::fill_from_sibling(c, input.link_id, input.student_id, &fields)?;
        repo::student::sync_issues_with_siblings(c, input.student_id, input.school_year, today)?;
        Ok(FillResult {
            filled: filled.iter().map(|f| f.label().to_string()).collect(),
        })
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanStarted {
    pub job_id: String,
}

/// 그 학년도 재학생에서 형제 후보를 다시 찾는다. 딴 갈래에서 돌고 진행 상황을 알린다.
#[tauri::command]
pub fn sibling_rescan(
    app: AppHandle,
    state: State<'_, AppState>,
    school_year: i32,
) -> AppResult<ScanStarted> {
    let db = state.db.clone();
    let job = Job::new(app, job::new_id("sibling"));
    let job_id = job.id().to_string();

    std::thread::spawn(move || {
        let today = chrono::Local::now().date_naive();
        let outcome = db.write(|c| {
            let out = repo::sibling::scan(c, school_year, today, |stage, done, total| {
                job.progress(stage, stage_label(stage), done, total)
            })?;

            // 표시를 다시 맞춘다. 판정 규칙은 sync_issues 한 곳에만 있다.
            job.progress("ISSUES", "확인 필요 정리", 0, 1);
            let ids: Vec<i64> = c
                .prepare("SELECT student_id FROM enrollments WHERE school_year = ?1")?
                .query_map([school_year], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?;
            let total = ids.len();
            let step = (total / 50).max(1);
            for (i, id) in ids.iter().enumerate() {
                repo::student::sync_issues(c, *id, school_year, today)?;
                if i % step == 0 {
                    job.progress("ISSUES", "확인 필요 정리", i, total);
                }
            }
            job.progress("ISSUES", "확인 필요 정리", total, total);
            Ok::<ScanResult, AppError>(out)
        });

        match outcome {
            Ok(result) => job.done(result),
            Err(e) => job.failed(e),
        }
    });

    Ok(ScanStarted { job_id })
}

fn stage_label(stage: &str) -> &'static str {
    match stage {
        "PAIR" => "견줄 학생 찾기",
        "COMPARE" => "보호자 정보 비교",
        "GUARDIAN" => "형제 정보 확인",
        _ => "처리 중",
    }
}
