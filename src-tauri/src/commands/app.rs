//! 앱 기본 정보 / 폴더 열기.

use serde::Serialize;
use tauri::State;

use crate::db::migrate;
use crate::error::{AppError, AppResult};
use crate::repo;
use crate::AppState;

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
    /// 초기 설정 완료 여부 (현재 학년도가 있으면 완료로 본다)
    pub setup_completed: bool,
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
        setup_completed: current_year.is_some(),
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
