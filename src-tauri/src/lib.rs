mod commands;
mod db;
mod domain;
pub mod error;
mod repo;

use std::sync::Arc;

use tauri::Manager;

use crate::db::Db;

pub struct AppState {
    pub db: Arc<Db>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // %APPDATA%\kr.school.studentroster\studentroster.db
            let dir = app.path().app_data_dir()?;
            let db_path = dir.join("studentroster.db");

            let db = Db::open(&db_path)?;
            log::info!(
                "DB ready: {} (schema v{})",
                db.path().display(),
                db.schema_version()?
            );

            app.manage(AppState { db: Arc::new(db) });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::app_info,
            commands::app::data_open_folder,
            // 설정 · 학년도
            commands::settings::settings_get,
            commands::settings::settings_save_school,
            commands::settings::school_year_create,
            commands::settings::school_year_set_current,
        ])
        .run(tauri::generate_context!())
        .expect("학생명단 관리 시스템을 시작하지 못했습니다.");
}
