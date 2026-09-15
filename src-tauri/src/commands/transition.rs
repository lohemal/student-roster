//! 학년도 전환 명령.
//!
//! 연 1회 돌리는 큰 작업이라 **한 번에 끝내지 않고 단계로 나눈다** —
//! 대상 확인 → 배정 자료 읽기 → 미리보기 → 적용. 앞 단계는 DB 를 건드리지 않는다.
//!
//! 적용은 **백업이 성공한 뒤에만** 시작하고, 한 트랜잭션 안에서 끝낸다.
//! 로그에는 학년도와 인원만 남긴다.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::error::{AppError, AppResult};
use crate::job::{self, Job};
use crate::repo::{self, transition::YearState};
use crate::transition::{self, AssignSet, Person, Plan};
use crate::AppState;

fn today() -> chrono::NaiveDate {
    chrono::Local::now().date_naive()
}

// ---------------------------------------------------------------
// 1단계 — 대상 확인
// ---------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YearInfo {
    pub year: i32,
    pub exists: bool,
    pub enrollments: i64,
    pub active: i64,
}

impl From<YearState> for YearInfo {
    fn from(s: YearState) -> Self {
        YearInfo {
            year: s.year,
            exists: s.exists,
            enrollments: s.enrollments,
            active: s.active,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetInfo {
    pub from: YearInfo,
    pub to: YearInfo,
    /// 이미 같은 전환을 돌린 적이 있으면 그 시각
    pub done_at: Option<String>,
    /// 원본 학년도 6학년 재학생 — 기본 졸업 대상
    pub graduating: Vec<Person>,
    /// 배정이 필요한 1~5학년 수
    pub promoting: usize,
}

/// 어느 학년도에서 어느 학년도로 넘어갈지. `from` 을 주지 않으면 현재 학년도.
#[tauri::command]
pub fn transition_target(
    state: State<'_, AppState>,
    from_year: Option<i32>,
    to_year: Option<i32>,
) -> AppResult<TargetInfo> {
    state.db.read(|c| {
        let from = match from_year {
            Some(y) => y,
            None => repo::settings::current_year(c)?.ok_or_else(|| {
                AppError::setup_required("먼저 설정에서 현재 학년도를 지정해 주세요.")
            })?,
        };
        let to = to_year.unwrap_or(from + 1);

        let seats = repo::transition::seats(c, from)?;
        let graduating: Vec<Person> = transition::graduation_candidates(c, from)?;
        let promoting = seats
            .iter()
            .filter(|s| crate::domain::transition::next_grade(s.grade).is_some())
            .count();

        Ok(TargetInfo {
            from: repo::transition::year_state(c, from)?.into(),
            to: repo::transition::year_state(c, to)?.into(),
            done_at: repo::transition::done_before(c, from, to)?,
            graduating,
            promoting,
        })
    })
}

// ---------------------------------------------------------------
// 2단계 — 진급 배정 자료
// ---------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssignInfo {
    pub id: String,
    pub file_name: String,
    pub sheet_name: String,
    pub header_row: usize,
    pub rows: usize,
    /// 찾지 못한 열 이름
    pub missing: Vec<String>,
}

/// 배정 파일을 읽어 둔다. 아직 아무것도 저장하지 않는다.
#[tauri::command]
pub fn transition_read_assign(
    state: State<'_, AppState>,
    path: String,
    from_year: i32,
    sheet: Option<String>,
) -> AppResult<AssignInfo> {
    let read = transition::assign::read(&path, sheet.as_deref())?;
    let file_name = std::path::Path::new(&path)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.clone());

    let id = job::new_id("assign");
    let info = AssignInfo {
        id: id.clone(),
        file_name: file_name.clone(),
        sheet_name: read.sheet_name.clone(),
        header_row: read.header_row,
        rows: read.rows.len(),
        missing: read.missing.iter().map(|s| s.to_string()).collect(),
    };
    state.transition.put(AssignSet {
        id,
        file_name,
        sheet_name: read.sheet_name,
        from_year,
        rows: read.rows,
    });
    // 파일 이름과 줄 수만 남긴다 — 학생 자료는 남기지 않는다
    log::info!("transition assign loaded: {} rows", info.rows);
    Ok(info)
}

/// 읽어 둔 배정 자료를 버린다.
#[tauri::command]
pub fn transition_clear_assign(state: State<'_, AppState>) -> AppResult<()> {
    state.transition.clear();
    Ok(())
}

// ---------------------------------------------------------------
// 3단계 — 미리보기
// ---------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanInput {
    pub from_year: i32,
    pub to_year: i32,
    /// 졸업에서 뺄 학생 (기본은 6학년 모두 졸업)
    #[serde(default)]
    pub exclude_graduation: Vec<i64>,
}

/// 무슨 일이 생길지 미리 센다. **DB 를 건드리지 않는다.**
#[tauri::command]
pub fn transition_preview(state: State<'_, AppState>, input: PlanInput) -> AppResult<Plan> {
    let assigns = state.transition.snapshot();
    check_assign_year(assigns.as_ref(), input.from_year)?;
    state.db.read(|c| {
        transition::build(
            c,
            input.from_year,
            input.to_year,
            assigns.as_ref(),
            &input.exclude_graduation,
            today(),
        )
    })
}

fn check_assign_year(assigns: Option<&AssignSet>, from_year: i32) -> AppResult<()> {
    if let Some(a) = assigns {
        if a.from_year != from_year {
            return Err(AppError::invalid(format!(
                "읽어 둔 배정 자료는 {}학년도 것입니다. 다시 읽어 주세요.",
                a.from_year
            )));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------
// 4단계 — 적용
// ---------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyInput {
    #[serde(flatten)]
    pub plan: PlanInput,
    /// 미리보기에서 받은 값. 그사이 명단이 바뀌었으면 거절한다.
    pub state_key: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyStarted {
    pub job_id: String,
    pub total: usize,
    /// 만들어 둔 백업 파일
    pub backup_path: String,
    pub backup_ms: u128,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyResult {
    #[serde(flatten)]
    pub summary: transition::Summary,
    pub backup_path: String,
    pub elapsed_ms: u128,
}

/// 전환을 시작한다. 바로 돌아오고, 끝은 `job://done` 으로 알린다.
///
/// 순서가 중요하다 — **백업이 성공한 뒤에** 트랜잭션을 연다. 백업에 실패하면
/// 아무것도 바꾸지 않는다.
#[tauri::command]
pub fn transition_apply(
    app: AppHandle,
    state: State<'_, AppState>,
    input: ApplyInput,
) -> AppResult<ApplyStarted> {
    let assigns = state.transition.snapshot();
    check_assign_year(assigns.as_ref(), input.plan.from_year)?;
    let today = today();

    // 1) 먼저 계획을 다시 세워 본다 — 막힐 일이면 백업도 만들지 않는다
    let plan = state.db.read(|c| {
        transition::build(
            c,
            input.plan.from_year,
            input.plan.to_year,
            assigns.as_ref(),
            &input.plan.exclude_graduation,
            today,
        )
    })?;
    check_fresh(&plan, &input.state_key)?;
    let total = plan.promote_count + plan.graduate_count;

    // 2) 백업 — 실패하면 전환을 시작하지 않는다
    let stamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let backup = state.db.backup_dir().join(format!(
        "before_year_transition_{}_to_{}_{stamp}.db",
        input.plan.from_year, input.plan.to_year
    ));
    let t0 = std::time::Instant::now();
    state.db.backup_to(&backup).map_err(|e| {
        AppError::new(
            "BACKUP_FAILED",
            "전환 전 백업을 만들지 못해 시작하지 않았습니다. 저장 공간을 확인해 주세요.",
        )
        .detail(e.detail.unwrap_or(e.user_message))
    })?;
    let backup_ms = t0.elapsed().as_millis();
    let backup_path = backup.display().to_string();

    // 3) 한 트랜잭션 — 중간에 실패하면 하나도 들어가지 않는다
    let db = state.db.clone();
    let job = Job::new(app, job::new_id("transition"));
    let job_id = job.id().to_string();
    let state_key = input.state_key.clone();
    let plan_input = input.plan.clone();
    let path_for_result = backup_path.clone();

    std::thread::spawn(move || {
        let started = std::time::Instant::now();
        let outcome = db.write(|c| {
            repo::settings::create_year(c, plan_input.to_year)?;
            // 판정을 트랜잭션 안에서 다시 — 미리보기와 결과가 어긋날 수 없다
            let fresh = transition::build(
                c,
                plan_input.from_year,
                plan_input.to_year,
                assigns.as_ref(),
                &plan_input.exclude_graduation,
                today,
            )?;
            check_fresh(&fresh, &state_key)?;
            transition::apply(c, &fresh, today, |stage, label, done, total| {
                job.progress(stage, label, done, total)
            })
        });

        match outcome {
            Ok(summary) => {
                log::info!(
                    "year transition {}->{}: promoted {}, graduated {}",
                    summary.from_year,
                    summary.to_year,
                    summary.promoted,
                    summary.graduated
                );
                job.done(ApplyResult {
                    summary,
                    backup_path: path_for_result,
                    elapsed_ms: started.elapsed().as_millis(),
                });
            }
            Err(e) => {
                log::warn!("year transition failed: {}", e.code);
                job.failed(e);
            }
        }
    });

    Ok(ApplyStarted {
        job_id,
        total,
        backup_path,
        backup_ms,
    })
}

/// 미리보기 이후 원본이 바뀌었거나 막을 일이 생겼는지 본다.
fn check_fresh(plan: &Plan, state_key: &str) -> AppResult<()> {
    if plan.state_key != state_key {
        return Err(AppError::new(
            "STALE",
            "미리보기를 만든 뒤 학생명단이 바뀌었습니다. 미리보기를 다시 만들어 주세요.",
        ));
    }
    if plan.blocked {
        return Err(AppError::invalid(
            "먼저 확인해야 할 것이 남아 있습니다. 전환을 시작하지 않았습니다.",
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------
// 기록
// ---------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryRow {
    pub id: i64,
    pub from_year: i32,
    pub to_year: i32,
    pub executed_at: String,
    pub summary: Option<String>,
}

#[tauri::command]
pub fn transition_history(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> AppResult<Vec<HistoryRow>> {
    let rows = state
        .db
        .read(|c| repo::transition::history(c, limit.unwrap_or(20).clamp(1, 100)))?;
    Ok(rows
        .into_iter()
        .map(|r| HistoryRow {
            id: r.id,
            from_year: r.from_year,
            to_year: r.to_year,
            executed_at: r.executed_at,
            summary: r.summary,
        })
        .collect())
}
