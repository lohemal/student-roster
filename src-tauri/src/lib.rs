mod commands;
mod db;
mod domain;
pub mod error;
mod export;
mod import;
mod job;
mod repo;

use std::sync::Arc;

use tauri::Manager;

use crate::db::Db;
use crate::import::SessionStore;

pub struct AppState {
    pub db: Arc<Db>,
    /// 분석해 둔 가져오기를 적용할 때까지 들고 있는 자리
    pub import: Arc<SessionStore>,
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

            app.manage(AppState {
                db: Arc::new(db),
                import: Arc::new(SessionStore::default()),
            });
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
            // 학생 · 명단
            commands::student::student_list,
            commands::student::student_get,
            commands::student::student_create,
            commands::student::student_update,
            commands::student::student_delete,
            commands::student::student_class_options,
            // 확인 필요
            commands::issue::issue_summary,
            commands::issue::issue_recompute,
            commands::issue::issue_list,
            // 학생 번호 재정렬
            commands::renumber::renumber_preview,
            commands::renumber::renumber_apply,
            // 전입 · 전출
            commands::transfer::transfer_search_students,
            commands::transfer::transfer_class_counts,
            commands::transfer::transfer_used_numbers,
            commands::transfer::transfer_list,
            commands::transfer::transfer_in,
            commands::transfer::transfer_out,
            commands::transfer::transfer_out_cancel,
            commands::transfer::transfer_past_out,
            // 통계
            commands::stats::stats_overview,
            // 엑셀 내보내기
            commands::export::export_columns,
            commands::export::export_preview,
            commands::export::export_run,
            commands::export::export_open_folder,
            // 엑셀 가져오기
            commands::import::import_inspect,
            commands::import::import_preview,
            commands::import::import_analyze,
            commands::import::import_apply,
            commands::import::import_history,
            // 주소 분류 · 규칙
            commands::address::address_category_list,
            commands::address::address_category_create,
            commands::address::address_category_rename,
            commands::address::address_category_delete,
            commands::address::address_rule_list,
            commands::address::address_rule_create,
            commands::address::address_rule_update,
            commands::address::address_rule_delete,
            commands::address::address_rule_preview,
            commands::address::address_rule_apply,
            commands::address::address_status,
            commands::address::address_set_manual,
            commands::address::address_reapply,
            // 본교 형제
            commands::sibling::sibling_list,
            commands::sibling::sibling_confirm,
            commands::sibling::sibling_reject,
            commands::sibling::sibling_reset,
            commands::sibling::sibling_fill,
            commands::sibling::sibling_rescan,
        ])
        .run(tauri::generate_context!())
        .expect("학생명단 관리 시스템을 시작하지 못했습니다.");
}
