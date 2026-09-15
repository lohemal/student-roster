//! 졸업생 명령.
//!
//! 졸업생도 같은 학생 자료를 본다. 따로 복사해 두지 않으므로 학생 상세 화면을
//! 그대로 쓸 수 있다.

use serde::Serialize;
use tauri::State;

use crate::error::AppResult;
use crate::repo;
use crate::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GradYear {
    pub school_year: i32,
    pub count: i64,
}

/// 졸업생이 있는 학년도. 최근 학년도부터.
#[tauri::command]
pub fn graduation_years(state: State<'_, AppState>) -> AppResult<Vec<GradYear>> {
    let rows = state.db.read(repo::graduation::years)?;
    Ok(rows
        .into_iter()
        .map(|r| GradYear {
            school_year: r.school_year,
            count: r.count,
        })
        .collect())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GradRow {
    pub student_id: i64,
    pub school_year: i32,
    pub name: String,
    pub gender: Option<String>,
    pub birth_date: Option<String>,
    pub birth_raw: Option<String>,
    pub grade: Option<i32>,
    pub class_name: Option<String>,
    pub class_no: Option<i32>,
    /// `6-가람`
    pub class_label: Option<String>,
    pub graduated_at: Option<String>,
    pub note: Option<String>,
}

fn to_row(r: repo::graduation::GradRow) -> GradRow {
    GradRow {
        student_id: r.student_id,
        school_year: r.school_year,
        name: r.name,
        gender: r.gender,
        birth_date: r.birth_date,
        birth_raw: r.birth_raw,
        grade: r.grade,
        class_name: r.class_name,
        class_no: r.class_no,
        class_label: r.class_label,
        graduated_at: r.graduated_at,
        note: r.note,
    }
}

/// 한 학년도 졸업생. 졸업 당시 학년·반·번호 그대로 보여 준다.
#[tauri::command]
pub fn graduation_list(
    state: State<'_, AppState>,
    school_year: i32,
    q: Option<String>,
) -> AppResult<Vec<GradRow>> {
    let rows = state
        .db
        .read(|c| repo::graduation::list(c, school_year, q.as_deref()))?;
    Ok(rows.into_iter().map(to_row).collect())
}

/// 잘못 만든 졸업 기록을 되돌린다. 돌아온 학년도를 알려 준다.
#[tauri::command]
pub fn graduation_cancel(state: State<'_, AppState>, student_id: i64) -> AppResult<i32> {
    let today = chrono::Local::now().date_naive();
    let year = state
        .db
        .write(|c| repo::graduation::cancel(c, student_id, today))?;
    // 개인정보는 남기지 않는다
    log::info!("graduation cancelled for {year}");
    Ok(year)
}
