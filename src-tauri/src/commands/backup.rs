//! 백업 · 복원 명령.
//!
//! 되돌릴 수 있게 하는 것이 목적이므로 **지금 자료를 지키는 쪽**으로만 움직인다.
//! 복원은 바로 덮어쓰지 않고 다음 실행 때 갈아 끼우도록 놓아 둔다(`db::backup`).
//!
//! 로그에 학생 자료를 남기지 않는다. 종류·인원 수·크기만 남긴다.

use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::db::backup::{self, Kind};
use crate::error::{AppError, AppResult};
use crate::AppState;

fn now() -> chrono::DateTime<chrono::Local> {
    chrono::Local::now()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupEntry {
    pub file_name: String,
    pub path: String,
    /// AUTO · MANUAL · BEFORE_RESTORE · BEFORE_MIGRATION · BEFORE_TRANSITION · OTHER
    pub kind: String,
    pub kind_label: String,
    pub created_at: String,
    pub size: u64,
    /// 열어서 확인했는가
    pub ok: bool,
    pub schema_version: Option<i32>,
    /// 그 백업에 들어 있는 학생 수 (개인정보가 아니다)
    pub students: Option<i64>,
    pub problem: Option<String>,
    /// 자동 정리가 지울 수 있는 종류인가
    pub cleaned_up: bool,
}

impl From<backup::Entry> for BackupEntry {
    fn from(e: backup::Entry) -> Self {
        BackupEntry {
            file_name: e.file_name,
            path: e.path,
            kind: e.kind.code().to_string(),
            kind_label: e.kind.label().to_string(),
            created_at: e.created_at,
            size: e.size,
            ok: e.check.ok,
            schema_version: e.check.schema_version,
            students: e.check.students,
            problem: e.check.problem,
            cleaned_up: e.kind.cleaned_up(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupStatus {
    pub folder: String,
    pub total: usize,
    pub auto: usize,
    pub kept_auto: usize,
    /// 가장 최근 백업 (종류를 가리지 않는다)
    pub latest: Option<BackupEntry>,
    /// 성하지 않은 백업 수
    pub broken: usize,
    /// 다음 실행 때 되돌릴 파일이 놓여 있는가
    pub restore_waiting: bool,
}

/// 설정 화면 맨 위에 보여 줄 요약.
#[tauri::command]
pub fn backup_status(state: State<'_, AppState>) -> AppResult<BackupStatus> {
    let dir = state.db.backup_dir();
    let all = backup::list(&dir)?;
    Ok(BackupStatus {
        folder: dir.display().to_string(),
        total: all.len(),
        auto: all.iter().filter(|e| e.kind == Kind::Auto).count(),
        kept_auto: backup::KEEP_AUTO,
        broken: all.iter().filter(|e| !e.check.ok).count(),
        latest: all.first().cloned().map(Into::into),
        restore_waiting: backup::restore_waiting(state.db.path()),
    })
}

/// 백업 목록. 파일마다 열어 보고 정상인지까지 확인한다.
#[tauri::command]
pub fn backup_list(state: State<'_, AppState>) -> AppResult<Vec<BackupEntry>> {
    Ok(backup::list(&state.db.backup_dir())?
        .into_iter()
        .map(Into::into)
        .collect())
}

/// [지금 백업]. 만든 뒤 바로 확인하고, 확인에 실패하면 그 파일을 지운다.
#[tauri::command]
pub fn backup_now(state: State<'_, AppState>) -> AppResult<BackupEntry> {
    Ok(backup::create(&state.db, Kind::Manual, now())?.into())
}

/// 백업 폴더를 탐색기로 연다. USB 로 옮겨 두려면 여기서 복사한다.
#[tauri::command]
pub fn backup_open_folder(state: State<'_, AppState>) -> AppResult<()> {
    let dir = state.db.backup_dir();
    std::fs::create_dir_all(&dir)?;
    super::app::open_in_explorer(&dir)
}

/// 다른 곳에 보관해 둔 파일을 고르면 **열어 보고** 어떤 자료인지 알려 준다.
#[tauri::command]
pub fn backup_check_file(path: String) -> AppResult<BackupEntry> {
    let p = PathBuf::from(&path);
    let file_name = p
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.clone());
    let meta = std::fs::metadata(&p).ok();
    let result = backup::check(&p);
    Ok(BackupEntry {
        kind: Kind::of_file(&file_name).code().to_string(),
        kind_label: Kind::of_file(&file_name).label().to_string(),
        cleaned_up: false,
        created_at: meta
            .as_ref()
            .and_then(|m| m.modified().ok())
            .map(|t| {
                chrono::DateTime::<chrono::Local>::from(t)
                    .format("%Y.%m.%d. %H:%M:%S")
                    .to_string()
            })
            .unwrap_or_default(),
        size: meta.map(|m| m.len()).unwrap_or(0),
        ok: result.ok,
        schema_version: result.schema_version,
        students: result.students,
        problem: result.problem,
        file_name,
        path,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreStaged {
    pub file_name: String,
    pub students: Option<i64>,
    pub schema_version: Option<i32>,
}

/// 복원을 준비한다. **아직 지금 자료를 건드리지 않는다.**
///
/// 고른 파일을 확인해 자리를 잡아 두고, 프로그램을 다시 켤 때 갈아 끼운다.
/// 그때 지금 자료를 `before_restore_*.db` 로 한 번 더 백업한다.
#[tauri::command]
pub fn restore_stage(state: State<'_, AppState>, path: String) -> AppResult<RestoreStaged> {
    let source = PathBuf::from(&path);
    let result = backup::stage_restore(state.db.path(), &source)?;
    let file_name = source
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or(path);
    Ok(RestoreStaged {
        file_name,
        students: result.students,
        schema_version: result.schema_version,
    })
}

/// 준비해 둔 복원을 물린다.
#[tauri::command]
pub fn restore_cancel(state: State<'_, AppState>) -> AppResult<bool> {
    backup::cancel_restore(state.db.path())
}

/// 프로그램을 끝낸다. 복원을 마치려면 사용자가 다시 켜야 한다.
///
/// 되돌리기가 걸린 상태에서 스스로 다시 켜지 않는 까닭: 자동 재시작이 어긋나면
/// 자료를 갈아 끼우는 도중에 두 개의 프로그램이 같은 파일을 볼 수 있다.
#[tauri::command]
pub fn app_quit(app: AppHandle) -> AppResult<()> {
    log::info!("quit requested");
    app.exit(0);
    Ok(())
}

/// 자료가 성한지 지금 확인한다.
#[tauri::command]
pub fn db_integrity(state: State<'_, AppState>) -> AppResult<String> {
    state.db.integrity()
}

/// 백업 폴더에 있는 파일 하나를 지운다. 사용자가 고른 것만 지운다.
#[tauri::command]
pub fn backup_delete(state: State<'_, AppState>, file_name: String) -> AppResult<()> {
    // 폴더 밖으로 나가지 못하게 이름만 받는다
    if file_name.contains(['/', '\\', ':']) || file_name.trim().is_empty() {
        return Err(AppError::invalid("백업 파일 이름이 올바르지 않습니다."));
    }
    let path = state.db.backup_dir().join(&file_name);
    if !path.exists() {
        return Err(AppError::not_found("백업 파일을 찾을 수 없습니다."));
    }
    std::fs::remove_file(&path)?;
    log::info!("backup deleted by user");
    Ok(())
}
