//! 내보내기 명령.
//!
//! **미리보기와 결과가 같은 계획에서 나온다.** 화면이 "3학년 5개 파일, 128명" 이라고
//! 보여 준 뒤 다른 파일이 만들어지면 사용자는 결과를 열어 보기 전까지 알 수 없다.
//! 그래서 두 명령이 똑같은 `build_plan` 하나를 지난다.
//!
//! 로그에는 학생 이름·연락처·주소를 남기지 않는다. 파일 수와 인원만 남긴다.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::domain::export::{Column, Grouping};
use crate::error::{AppError, AppResult};
use crate::export::{self, preset, preset::Preset, ExportPlan, Warning};
use crate::repo::{self, student::ListFilter};
use crate::AppState;

/// 미리보기에 보여 줄 줄 수. 눈으로 확인할 만큼만 내려보낸다.
const SAMPLE_ROWS: usize = 5;

// ---------------------------------------------------------------
// 열 목록
// ---------------------------------------------------------------

/// 화면이 그릴 열 하나.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnInfo {
    pub key: String,
    pub label: String,
    /// 개인정보인가 — 화면에서 사용자에게 알려 준다
    pub personal: bool,
    /// 처음 열었을 때 켜져 있는 열인가
    pub default_on: bool,
}

/// 고를 수 있는 열 전체. **이름과 차례의 원본은 Rust 하나**이고 화면은 이것을 받아 그린다.
#[tauri::command]
pub fn export_columns() -> Vec<ColumnInfo> {
    Column::ALL
        .into_iter()
        .map(|c| ColumnInfo {
            key: c.key().to_string(),
            label: c.label().to_string(),
            personal: c.is_personal(),
            default_on: Column::DEFAULT.contains(&c),
        })
        .collect()
}

// ---------------------------------------------------------------
// 요청
// ---------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    pub preset: Preset,
    /// 누구를 내보낼지. 학생명단과 **같은 조건**이다.
    pub filter: ListFilter,
    /// 사용자 지정에서 고른 열. 고른 차례가 Excel 의 차례가 된다.
    #[serde(default)]
    pub columns: Vec<String>,
    /// 몇 개 파일로 나눌지. 필터와는 다른 이야기다.
    #[serde(default)]
    pub grouping: Option<Grouping>,
}

impl ExportRequest {
    fn grouping(&self) -> Grouping {
        self.grouping.unwrap_or(Grouping::All)
    }

    fn columns(&self) -> AppResult<Vec<Column>> {
        if self.columns.is_empty() {
            return Err(AppError::invalid("내보낼 열을 하나 이상 골라 주세요."));
        }
        self.columns
            .iter()
            .map(|k| {
                Column::parse(k)
                    .ok_or_else(|| AppError::invalid(format!("알 수 없는 열입니다: {k}")))
            })
            .collect()
    }
}

fn build_plan(state: &State<'_, AppState>, req: &ExportRequest) -> AppResult<ExportPlan> {
    let rows = state.db.read(|c| repo::export::rows(c, &req.filter))?;
    let year = req.filter.school_year;

    Ok(match req.preset {
        Preset::Custom => preset::custom(&rows, &req.columns()?, year, req.grouping()),
        Preset::Schooljongi => preset::schooljongi(&rows, year),
        Preset::Alime => preset::alime(&rows, year, req.grouping()),
        // 진급 배정 양식 — 학년도 전환 화면이 쓴다. 열은 사람이 고르지 않는다.
        Preset::Promotion => preset::promotion(&rows, year),
    })
}

// ---------------------------------------------------------------
// 미리보기
// ---------------------------------------------------------------

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetInfo {
    pub name: String,
    pub students: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
    pub file_name: String,
    pub label: String,
    pub students: usize,
    pub sheets: Vec<SheetInfo>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportPreview {
    pub preset_label: String,
    pub grouping_label: String,
    /// Excel 머리글 — 실제로 파일에 들어갈 것과 같다
    pub headers: Vec<String>,
    /// 첫 몇 줄. 파일을 만들기 전에 눈으로 확인한다.
    pub sample: Vec<Vec<String>>,
    pub files: Vec<FileInfo>,
    /// 실제로 파일에 들어가는 학생 수
    pub students: usize,
    /// 조건에 든 학생 수 (빠지는 학생을 포함)
    pub matched: usize,
    pub warnings: Vec<Warning>,
    /// 파일이 하나인가 — 화면이 저장 창을 띄울지 폴더 창을 띄울지 고른다
    pub single_file: bool,
    /// 확장자를 뺀 기본 파일 이름 (하나일 때)
    pub default_file_name: Option<String>,
}

/// 만들어질 파일을 미리 본다. 아무것도 쓰지 않는다.
#[tauri::command]
pub fn export_preview(
    state: State<'_, AppState>,
    request: ExportRequest,
) -> AppResult<ExportPreview> {
    let plan = build_plan(&state, &request)?;
    let first = plan.files.first().and_then(|f| f.sheets.first());

    Ok(ExportPreview {
        preset_label: request.preset.label().to_string(),
        grouping_label: request.grouping().label().to_string(),
        headers: first.map(|s| s.headers.clone()).unwrap_or_default(),
        sample: first
            .map(|s| s.rows.iter().take(SAMPLE_ROWS).cloned().collect())
            .unwrap_or_default(),
        files: plan
            .files
            .iter()
            .map(|f| FileInfo {
                file_name: f.file_name.clone(),
                label: f.label.clone(),
                students: f.students(),
                sheets: f
                    .sheets
                    .iter()
                    .map(|s| SheetInfo {
                        name: s.name.clone(),
                        students: s.students(),
                    })
                    .collect(),
            })
            .collect(),
        students: plan.students,
        matched: plan.matched,
        warnings: plan.warnings.clone(),
        single_file: plan.is_single_file(),
        default_file_name: plan.files.first().map(|f| f.file_name.clone()),
    })
}

// ---------------------------------------------------------------
// 내보내기
// ---------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunRequest {
    #[serde(flatten)]
    pub request: ExportRequest,
    /// 파일이 하나면 저장할 파일 경로, 여럿이면 폴더 경로
    pub target: String,
    /// 같은 이름이 있어도 덮어쓸지. 사용자가 확인한 뒤에만 true 다.
    #[serde(default)]
    pub overwrite: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    pub paths: Vec<String>,
    /// 만든 파일이 있는 폴더 — [폴더 열기] 에 쓴다
    pub folder: String,
    pub files: usize,
    pub students: usize,
}

/// 파일을 만든다.
#[tauri::command]
pub fn export_run(state: State<'_, AppState>, request: RunRequest) -> AppResult<ExportResult> {
    let plan = build_plan(&state, &request.request)?;
    if plan.files.is_empty() {
        return Err(AppError::invalid("내보낼 학생이 없습니다."));
    }

    let target = PathBuf::from(&request.target);
    if !request.overwrite {
        // 덮어쓰기는 사용자가 정한다. 말없이 지우지 않는다.
        let exists: Vec<String> = export::target_paths(&plan, &target)
            .into_iter()
            .filter(|p| p.exists())
            .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .collect();
        if !exists.is_empty() {
            return Err(AppError::new(
                "EXPORT_EXISTS",
                format!("같은 이름의 파일이 {}개 이미 있습니다. 덮어쓸까요?", exists.len()),
            )
            .detail(exists.join(", ")));
        }
    }

    let made = export::write_plan(&plan, &target)?;
    let folder = made
        .first()
        .and_then(|p| p.parent())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| target.clone());

    Ok(ExportResult {
        paths: made.iter().map(|p| p.display().to_string()).collect(),
        folder: folder.display().to_string(),
        files: made.len(),
        students: plan.students,
    })
}

/// 만든 파일이 있는 폴더를 탐색기로 연다.
///
/// 어디에 저장됐는지 찾지 못해 다시 내보내는 일이 없도록, 끝난 화면에서 바로 연다.
#[tauri::command]
pub fn export_open_folder(path: String) -> AppResult<()> {
    let p = PathBuf::from(&path);
    let dir = if p.is_dir() {
        p
    } else {
        p.parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| AppError::invalid("폴더를 찾을 수 없습니다."))?
    };
    if !dir.exists() {
        return Err(AppError::not_found("폴더를 찾을 수 없습니다."));
    }
    super::app::open_in_explorer(&dir)
}
