//! 설정 · 학년도 명령.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::{AppError, AppResult};
use crate::repo;
use super::today;
use crate::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    pub school_name: String,
    pub current_year: Option<i32>,
    pub years: Vec<SchoolYearRow>,
    pub db_path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchoolYearRow {
    pub year: i32,
    pub is_current: bool,
    pub student_count: i64,
    pub created_at: String,
}

#[tauri::command]
pub fn settings_get(state: State<'_, AppState>) -> AppResult<SettingsView> {
    let db = &state.db;
    db.read(|c| {
        Ok(SettingsView {
            school_name: repo::settings::get(c, "school_name")?.unwrap_or_default(),
            current_year: repo::settings::current_year(c)?,
            years: repo::settings::list_years(c, today())?
                .into_iter()
                .map(|y| SchoolYearRow {
                    year: y.year,
                    is_current: y.is_current,
                    student_count: y.student_count,
                    created_at: y.created_at,
                })
                .collect(),
            db_path: db.path().display().to_string(),
        })
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchoolInput {
    pub school_name: String,
}

#[tauri::command]
pub fn settings_save_school(state: State<'_, AppState>, input: SchoolInput) -> AppResult<()> {
    let name = input.school_name.trim();
    if name.is_empty() {
        return Err(AppError::invalid("학교 이름을 입력해 주세요."));
    }
    if name.chars().count() > 50 {
        return Err(AppError::invalid("학교 이름은 50자 이내로 입력해 주세요."));
    }
    state
        .db
        .write(|c| repo::settings::set(c, "school_name", name))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YearInput {
    pub year: i32,
    #[serde(default)]
    pub set_current: bool,
}

#[tauri::command]
pub fn school_year_create(state: State<'_, AppState>, input: YearInput) -> AppResult<()> {
    validate_year(input.year)?;
    state.db.write(|c| {
        repo::settings::create_year(c, input.year)?;
        if input.set_current {
            repo::settings::set_current_year(c, input.year)?;
        }
        Ok(())
    })
}

#[tauri::command]
pub fn school_year_set_current(state: State<'_, AppState>, year: i32) -> AppResult<()> {
    validate_year(year)?;
    state
        .db
        .write(|c| repo::settings::set_current_year(c, year))
}

// ---------------------------------------------------------------
// 학년도 삭제
// ---------------------------------------------------------------

/// 지우면 무엇이 사라지고 무엇이 남는지. **아무것도 바꾸지 않는다.**
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YearDeleteImpact {
    pub year: i32,
    pub deletable: bool,
    pub refusal_code: Option<String>,
    pub refusal_message: Option<String>,

    // 지워지는 것
    pub enrollments: i64,
    pub events: i64,
    pub moves: i64,
    pub renumber_ops: i64,
    pub imports: i64,
    pub transitions: i64,
    // 다시 계산되는 것
    pub issues: i64,
    // 남는 것
    pub students: i64,
    pub students_without_other_year: i64,
    pub other_enrollments: i64,
    pub graduations: i64,
    pub graduations_from_transition: i64,
    pub graduation_year: Option<i32>,
    pub sibling_links: i64,
}

impl From<repo::year::Impact> for YearDeleteImpact {
    fn from(i: repo::year::Impact) -> Self {
        YearDeleteImpact {
            year: i.year,
            deletable: i.deletable,
            refusal_code: i.refusal_code,
            refusal_message: i.refusal_message,
            enrollments: i.enrollments,
            events: i.events,
            moves: i.moves,
            renumber_ops: i.renumber_ops,
            imports: i.imports,
            transitions: i.transitions,
            issues: i.issues,
            students: i.students,
            students_without_other_year: i.students_without_other_year,
            other_enrollments: i.other_enrollments,
            graduations: i.graduations,
            graduations_from_transition: i.graduations_from_transition,
            graduation_year: i.graduation_year,
            sibling_links: i.sibling_links,
        }
    }
}

/// 삭제 전 영향 범위. 버튼을 누르면 바로 지우지 않고 이것을 먼저 보여 준다.
#[tauri::command]
pub fn school_year_delete_impact(
    state: State<'_, AppState>,
    year: i32,
) -> AppResult<YearDeleteImpact> {
    validate_year(year)?;
    state.db.read(|c| Ok(repo::year::impact(c, year)?.into()))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YearDeleteResult {
    pub year: i32,
    pub enrollments: i64,
    pub events: i64,
    pub renumber_ops: i64,
    pub imports: i64,
    pub transitions: i64,
    pub resynced: i64,
    /// 지우기 직전에 만들어 둔 백업 파일
    pub backup_path: String,
}

/// 학년도를 지운다.
///
/// 순서가 중요하다 — **백업이 성공한 뒤에** 트랜잭션을 연다. 백업에 실패하면
/// 아무것도 지우지 않는다. 이 백업은 자동 정리(AUTO 30개) 대상이 아니다.
#[tauri::command]
pub fn school_year_delete(
    state: State<'_, AppState>,
    year: i32,
) -> AppResult<YearDeleteResult> {
    validate_year(year)?;

    // 1) 막힐 일이면 백업도 만들지 않는다
    let impact = state.db.read(|c| repo::year::impact(c, year))?;
    if !impact.deletable {
        let message = impact
            .refusal_message
            .unwrap_or_else(|| format!("{year}학년도는 삭제할 수 없습니다."));
        return Err(match impact.refusal_code.as_deref() {
            Some("NOT_FOUND") => AppError::not_found(message),
            _ => AppError::invalid(message),
        });
    }

    // 2) 백업 — 되돌릴 곳이 생긴 뒤에만 지운다
    let stamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let backup = state
        .db
        .backup_dir()
        .join(format!("before_year_delete_{year}_{stamp}.db"));
    state.db.backup_to(&backup).map_err(|e| {
        AppError::new(
            "BACKUP_FAILED",
            "삭제 전 백업을 만들지 못해 지우지 않았습니다. 저장 공간을 확인해 주세요.",
        )
        .detail(e.detail.unwrap_or(e.user_message))
    })?;

    // 3) 한 트랜잭션 — 중간에 실패하면 하나도 지워지지 않는다
    let out = state.db.write(|c| repo::year::delete(c, year, today()))?;

    Ok(YearDeleteResult {
        year: out.year,
        enrollments: out.enrollments,
        events: out.events,
        renumber_ops: out.renumber_ops,
        imports: out.imports,
        transitions: out.transitions,
        resynced: out.resynced,
        backup_path: backup.display().to_string(),
    })
}

fn validate_year(year: i32) -> AppResult<()> {
    if !(2000..=2100).contains(&year) {
        return Err(AppError::invalid("학년도는 2000~2100 사이의 연도로 입력해 주세요."));
    }
    Ok(())
}
