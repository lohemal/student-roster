//! 통계 명령.
//!
//! 화면이 쓰는 모든 표를 **한 번에** 만들어 준다. 표마다 따로 물으면 그 사이에 자료가
//! 바뀌어 한 화면 안에서 숫자가 어긋날 수 있다. 같은 순간의 자료로 함께 센다.

use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::{AppError, AppResult};
use crate::export::{self, stats as stats_export};
use crate::repo::{
    self,
    stats::{
        AddressQuality, AddressTable, ClassCount, Consistency, Counts, GradeRow, StatFilter,
    },
};
use super::today;
use crate::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsOverview {
    /// 이 숫자들이 어느 학년도 것인지
    pub school_year: i32,
    /// 지금 학년도인가 — 아니면 화면이 '학년도 최종 재적 기준' 이라고 알려 준다
    pub is_current_year: bool,
    pub totals: Counts,
    pub by_grade: Vec<GradeRow>,
    pub by_class: Vec<ClassCount>,
    pub address: AddressTable,
    pub address_quality: AddressQuality,
    /// 모든 표의 합계가 서로 맞는지
    pub consistency: Consistency,
}

/// 화면과 Excel 이 **같은 함수**를 지난다. 집계가 두 군데 있으면 언젠가 다른
/// 숫자를 말한다.
fn overview(c: &Connection, filter: &StatFilter) -> AppResult<StatsOverview> {
    let current = repo::settings::current_year(c)?;
    Ok(StatsOverview {
        school_year: filter.school_year,
        is_current_year: current == Some(filter.school_year),
        totals: repo::stats::totals(c, filter, today())?,
        by_grade: repo::stats::by_grade(c, filter, today())?,
        by_class: repo::stats::by_class(c, filter, today())?,
        address: repo::stats::by_address(c, filter, today())?,
        address_quality: repo::stats::address_quality(c, filter, today())?,
        consistency: repo::stats::check_consistency(c, filter, today())?,
    })
}

/// 고른 조건으로 통계 전체를 한 번에 센다.
#[tauri::command]
pub fn stats_overview(
    state: State<'_, AppState>,
    filter: StatFilter,
) -> AppResult<StatsOverview> {
    state.db.read(|c| overview(c, &filter))
}

// ---------------------------------------------------------------
// Excel 다운로드
// ---------------------------------------------------------------

/// 고를 수 있는 통계 한 가지. 이름의 원본은 Rust 하나다.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsSheetInfo {
    pub key: String,
    pub label: String,
}

#[tauri::command]
pub fn stats_export_sheets() -> Vec<StatsSheetInfo> {
    stats_export::Kind::ALL
        .into_iter()
        .map(|k| StatsSheetInfo {
            key: k.code().to_string(),
            label: k.label().to_string(),
        })
        .collect()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsExportRequest {
    /// 화면에 걸려 있는 조건 그대로
    pub filter: StatFilter,
    /// 고른 통계. 하나 이상이라야 한다.
    pub sheets: Vec<stats_export::Kind>,
}

impl StatsExportRequest {
    fn picked(&self) -> AppResult<&[stats_export::Kind]> {
        if self.sheets.is_empty() {
            return Err(AppError::invalid("내보낼 통계를 하나 이상 골라 주세요."));
        }
        Ok(&self.sheets)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsExportPreview {
    /// 확장자를 뺀 기본 파일 이름
    pub default_file_name: String,
    /// 만들어질 시트 이름 (고른 차례가 아니라 화면 차례)
    pub sheet_names: Vec<String>,
    /// 센 학생 수 — 화면 합계와 같아야 한다
    pub students: i64,
}

/// 무엇이 만들어질지 미리 본다. 아무것도 쓰지 않는다.
#[tauri::command]
pub fn stats_export_preview(
    state: State<'_, AppState>,
    request: StatsExportRequest,
) -> AppResult<StatsExportPreview> {
    let picked = request.picked()?;
    let data = state.db.read(|c| overview(c, &request.filter))?;
    let plan = stats_export::plan(&source(&data), picked);
    let file = &plan.files[0];

    Ok(StatsExportPreview {
        default_file_name: file.file_name.clone(),
        sheet_names: file.sheets.iter().map(|s| s.name.clone()).collect(),
        students: data.totals.total,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsExportRun {
    #[serde(flatten)]
    pub request: StatsExportRequest,
    /// 저장할 파일 경로 (`.xlsx` 포함)
    pub target: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsExportResult {
    pub path: String,
    pub folder: String,
    pub sheets: usize,
}

/// 통계 Excel 을 만든다. 통계는 나란히 견주는 자료라 **파일 하나에 시트 여럿**이다.
#[tauri::command]
pub fn stats_export_run(
    state: State<'_, AppState>,
    request: StatsExportRun,
) -> AppResult<StatsExportResult> {
    let picked = request.request.picked()?;
    let data = state.db.read(|c| overview(c, &request.request.filter))?;
    let plan = stats_export::plan(&source(&data), picked);

    // 저장 창이 이미 덮어쓸지 물었으므로 여기서 다시 묻지 않는다
    let target = PathBuf::from(&request.target);
    let made = export::write_plan(&plan, &target)?;
    let path = made
        .first()
        .cloned()
        .ok_or_else(|| AppError::invalid("파일을 만들지 못했습니다."))?;
    let folder = path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| target.clone());

    Ok(StatsExportResult {
        path: path.display().to_string(),
        folder: folder.display().to_string(),
        sheets: plan.files[0].sheets.len(),
    })
}

/// 센 결과를 그대로 넘긴다 — 여기서 다시 세지 않는다.
fn source(d: &StatsOverview) -> stats_export::Source<'_> {
    stats_export::Source {
        school_year: d.school_year,
        totals: &d.totals,
        by_grade: &d.by_grade,
        by_class: &d.by_class,
        address: &d.address,
    }
}
