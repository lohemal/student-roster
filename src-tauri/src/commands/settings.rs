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

fn validate_year(year: i32) -> AppResult<()> {
    if !(2000..=2100).contains(&year) {
        return Err(AppError::invalid("학년도는 2000~2100 사이의 연도로 입력해 주세요."));
    }
    Ok(())
}
