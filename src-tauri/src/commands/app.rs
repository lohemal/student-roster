//! 앱 기본 정보 / 폴더 열기 / 시작할 때 알릴 것.

use serde::Serialize;
use tauri::State;

use crate::db::migrate;
use crate::error::{AppError, AppResult};
use crate::repo;
use crate::AppState;

/// 자료 구조 버전 확인 주기 — 하루 한 번이면 충분하다.
const UPDATE_CHECK_HOURS: i64 = 24;

/// 설정 저장소에 남기는 마지막 업데이트 확인 시각.
const UPDATE_CHECKED_KEY: &str = "update_checked_at";

/// 시작할 때 사용자에게 알려야 할 일.
///
/// 복원이 이루어졌거나, 자료가 성하지 않거나, 되돌리려던 파일을 쓸 수 없었던 경우다.
/// **개인정보는 담지 않는다** — 무슨 일이 있었는지만 적는다.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupNote {
    /// RESTORED · RESTORE_REJECTED · INTEGRITY · BACKUP_FAILED
    pub kind: String,
    pub tone: String,
    pub message: String,
}

impl StartupNote {
    pub fn new(kind: &str, tone: &str, message: impl Into<String>) -> Self {
        StartupNote {
            kind: kind.into(),
            tone: tone.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    /// 앱 버전 (Cargo.toml)
    pub app_version: String,
    /// 현재 DB 스키마 버전
    pub schema_version: i32,
    /// 이 앱 빌드가 지원하는 최신 스키마 버전
    pub latest_schema_version: i32,
    /// 자료 파일 경로 (설정 화면의 '자료 위치 열기'에서 사용)
    pub db_path: String,
    /// 백업 폴더
    pub backup_dir: String,
    /// 초기 설정 완료 여부 (현재 학년도가 있으면 완료로 본다)
    pub setup_completed: bool,
    /// 시작할 때 있었던 일 (복원·무결성 등)
    pub notes: Vec<StartupNote>,
}

#[tauri::command]
pub fn app_info(state: State<'_, AppState>) -> AppResult<AppInfo> {
    let db = &state.db;

    let (schema_version, current_year) = db.read(|c| {
        let v = migrate::current_version(c)?;
        let y = repo::settings::current_year(c)?;
        Ok((v, y))
    })?;

    Ok(AppInfo {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        schema_version,
        latest_schema_version: migrate::latest_version(),
        db_path: db.path().display().to_string(),
        backup_dir: db.backup_dir().display().to_string(),
        setup_completed: current_year.is_some(),
        notes: state.notes(),
    })
}

/// 자료 폴더를 탐색기로 연다.
#[tauri::command]
pub fn data_open_folder(state: State<'_, AppState>) -> AppResult<()> {
    let dir = state
        .db
        .path()
        .parent()
        .ok_or_else(|| AppError::internal("자료 폴더를 찾을 수 없습니다."))?
        .to_path_buf();
    std::fs::create_dir_all(&dir)?;
    open_in_explorer(&dir)
}

pub(crate) fn open_in_explorer(dir: &std::path::Path) -> AppResult<()> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer").arg(dir).spawn()?;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = dir;
        Err(AppError::new("UNSUPPORTED", "이 운영체제에서는 폴더 열기를 지원하지 않습니다."))
    }
}

// ---------------------------------------------------------------
// 업데이트 확인 주기
// ---------------------------------------------------------------

/// 자동으로 업데이트를 확인할 때가 되었는가.
///
/// 앱을 열 때마다 네트워크를 기다리지 않는다. 하루에 한 번이면 충분하고,
/// 확인에 실패해도 프로그램 사용에는 아무 영향이 없다.
#[tauri::command]
pub fn update_check_due(state: State<'_, AppState>) -> AppResult<bool> {
    let last = state.db.read(|c| repo::settings::get(c, UPDATE_CHECKED_KEY))?;
    let Some(last) = last else {
        return Ok(true);
    };
    let parsed = chrono::NaiveDateTime::parse_from_str(&last, "%Y-%m-%d %H:%M:%S").ok();
    let Some(parsed) = parsed else {
        return Ok(true);
    };
    let hours = chrono::Local::now()
        .naive_local()
        .signed_duration_since(parsed)
        .num_hours();
    Ok(hours >= UPDATE_CHECK_HOURS || hours < 0)
}

/// 방금 확인했다고 적어 둔다.
#[tauri::command]
pub fn update_mark_checked(state: State<'_, AppState>) -> AppResult<()> {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    state
        .db
        .write(|c| repo::settings::set(c, UPDATE_CHECKED_KEY, &now))
}
