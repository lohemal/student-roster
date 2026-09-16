mod commands;
mod db;
mod domain;
pub mod error;
mod export;
mod import;
mod job;
mod repo;
mod transition;

use std::sync::{Arc, Mutex};

use tauri::Manager;

use crate::commands::app::StartupNote;
use crate::db::backup;
use crate::db::Db;
use crate::import::SessionStore;
use crate::transition::AssignStore;

pub struct AppState {
    pub db: Arc<Db>,
    /// 분석해 둔 가져오기를 적용할 때까지 들고 있는 자리
    pub import: Arc<SessionStore>,
    /// 읽어 둔 진급 배정 자료를 적용할 때까지 들고 있는 자리
    pub transition: Arc<AssignStore>,
    /// 시작할 때 있었던 일 (복원·무결성). 화면이 한 번 보여 준다.
    pub startup: Mutex<Vec<StartupNote>>,
}

impl AppState {
    pub fn notes(&self) -> Vec<StartupNote> {
        self.startup
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            // %APPDATA%\kr.school.studentroster\studentroster.db
            // 설치 폴더와 따로다 — 프로그램을 지우거나 새로 깔아도 자료는 남는다.
            let dir = app.path().app_data_dir()?;
            let db_path = dir.join("studentroster.db");
            let now = chrono::Local::now();
            let mut notes: Vec<StartupNote> = Vec::new();

            // 1) 되돌리기가 걸려 있으면 **연결을 열기 전에** 갈아 끼운다
            match backup::apply_pending(&db_path, now) {
                Ok(Some(message)) => notes.push(StartupNote::new("RESTORED", "success", message)),
                Ok(None) => {}
                Err(e) => notes.push(StartupNote::new(
                    "RESTORE_REJECTED",
                    "error",
                    e.user_message.clone(),
                )),
            }

            let db = Db::open(&db_path)?;
            log::info!(
                "DB ready: {} (schema v{})",
                db.path().display(),
                db.schema_version()?
            );

            // 2) 자료가 성한가
            let sound = match db.integrity() {
                Ok(v) if v == "ok" => true,
                Ok(v) => {
                    log::warn!("integrity check failed");
                    notes.push(StartupNote::new(
                        "INTEGRITY",
                        "error",
                        format!(
                            "자료 파일에 문제가 있습니다({v}). 새로 저장하기 전에 설정 화면에서 \
                             백업을 확인하고, 필요하면 최근 백업으로 되돌려 주세요."
                        ),
                    ));
                    false
                }
                Err(e) => {
                    notes.push(StartupNote::new("INTEGRITY", "error", e.user_message.clone()));
                    false
                }
            };

            // 3) 하루 한 번 자동 백업 — **성할 때만** 만든다.
            //    손상된 자료로 뜬 백업이 멀쩡한 지난 백업을 밀어내면 되돌릴 곳이 사라진다.
            if sound {
                let started = std::time::Instant::now();
                match backup::auto_if_needed(&db, now.date_naive(), now) {
                    Ok(Some(_)) => log::info!("auto backup done in {:?}", started.elapsed()),
                    Ok(None) => {}
                    Err(e) => {
                        log::warn!("auto backup failed: {}", e.code);
                        notes.push(StartupNote::new(
                            "BACKUP_FAILED",
                            "warn",
                            format!("자동 백업을 만들지 못했습니다. {}", e.user_message),
                        ));
                    }
                }
            }

            app.manage(AppState {
                db: Arc::new(db),
                import: Arc::new(SessionStore::default()),
                transition: Arc::new(AssignStore::default()),
                startup: Mutex::new(notes),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app::app_info,
            commands::app::data_open_folder,
            commands::app::update_check_due,
            commands::app::update_mark_checked,
            // 백업 · 복원
            commands::backup::backup_status,
            commands::backup::backup_list,
            commands::backup::backup_now,
            commands::backup::backup_delete,
            commands::backup::backup_open_folder,
            commands::backup::backup_check_file,
            commands::backup::restore_stage,
            commands::backup::restore_cancel,
            commands::backup::db_integrity,
            commands::backup::app_quit,
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
            // 학년도 전환 · 졸업생
            commands::transition::transition_target,
            commands::transition::transition_read_assign,
            commands::transition::transition_clear_assign,
            commands::transition::transition_preview,
            commands::transition::transition_apply,
            commands::transition::transition_history,
            commands::graduation::graduation_years,
            commands::graduation::graduation_list,
            commands::graduation::graduation_cancel,
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
            commands::sibling::sibling_candidates,
            commands::sibling::sibling_confirm_many,
            commands::sibling::sibling_reject,
            commands::sibling::sibling_reset,
            commands::sibling::sibling_fill,
            commands::sibling::sibling_rescan,
        ])
        .run(tauri::generate_context!())
        .expect("학생명단 관리 시스템을 시작하지 못했습니다.");
}
