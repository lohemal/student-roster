//! 통계 명령.
//!
//! 화면이 쓰는 모든 표를 **한 번에** 만들어 준다. 표마다 따로 물으면 그 사이에 자료가
//! 바뀌어 한 화면 안에서 숫자가 어긋날 수 있다. 같은 순간의 자료로 함께 센다.

use serde::Serialize;
use tauri::State;

use crate::error::AppResult;
use crate::repo::{
    self,
    stats::{
        AddressQuality, AddressTable, ClassCount, Consistency, Counts, GradeRow, StatFilter,
    },
};
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

/// 고른 조건으로 통계 전체를 한 번에 센다.
#[tauri::command]
pub fn stats_overview(
    state: State<'_, AppState>,
    filter: StatFilter,
) -> AppResult<StatsOverview> {
    state.db.read(|c| {
        let current = repo::settings::current_year(c)?;
        Ok(StatsOverview {
            school_year: filter.school_year,
            is_current_year: current == Some(filter.school_year),
            totals: repo::stats::totals(c, &filter)?,
            by_grade: repo::stats::by_grade(c, &filter)?,
            by_class: repo::stats::by_class(c, &filter)?,
            address: repo::stats::by_address(c, &filter)?,
            address_quality: repo::stats::address_quality(c, &filter)?,
            consistency: repo::stats::check_consistency(c, &filter)?,
        })
    })
}
