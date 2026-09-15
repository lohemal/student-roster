//! 엑셀 학생명단 가져오기.
//!
//! 흐름은 **읽기 → 짝짓기 → 분석 → (사람이 확인) → 적용** 이다.
//! 분석까지는 DB를 건드리지 않는다. 사용자가 무엇이 바뀌는지 보고 정한 뒤에만 저장한다.
//!
//! ```text
//! excel     파일·시트 읽기
//! mapping   엑셀 열 ↔ 프로그램 항목 짝짓기 (별칭표로 추측, 확정은 사람이)
//! row       한 줄에서 값 꺼내기
//! analyze   기존 학생과 견주어 추가/갱신/중복 의심 판정
//! apply     한 트랜잭션으로 저장
//! ```

pub mod analyze;
pub mod apply;
pub mod excel;
pub mod mapping;
pub mod row;

#[cfg(test)]
#[path = "real_file_tests.rs"]
mod real_file_tests;

use std::sync::Mutex;

use serde::Serialize;

use crate::error::{AppError, AppResult};
use mapping::Mapping;
use row::ParsedRow;

/// 분석해 둔 것을 적용할 때까지 들고 있는 자리.
///
/// 엑셀을 두 번 읽지 않고, 화면이 되돌려 보낸 값을 그대로 믿지도 않기 위해서다.
/// 한 번에 한 가져오기만 진행한다 — 실수로 두 번 누르는 것도 여기서 막힌다.
pub struct Session {
    pub id: String,
    pub file_name: String,
    pub sheet_name: String,
    pub school_year: i32,
    pub mapping: Mapping,
    pub rows: Vec<ParsedRow>,
    /// 지금 저장 중인가
    pub running: bool,
}

#[derive(Default)]
pub struct SessionStore(pub Mutex<Option<Session>>);

impl SessionStore {
    pub fn put(&self, s: Session) {
        let mut g = self.0.lock().unwrap_or_else(|e| e.into_inner());
        *g = Some(s);
    }

    /// 저장을 시작한다. 이미 돌고 있으면 막는다.
    pub fn start(&self, id: &str) -> AppResult<Session> {
        let mut g = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let s = g.as_mut().ok_or_else(|| {
            AppError::invalid("분석 결과가 없습니다. 파일을 다시 골라 주세요.")
        })?;
        if s.id != id {
            return Err(AppError::invalid(
                "분석 결과가 바뀌었습니다. 분석을 다시 해 주세요.",
            ));
        }
        if s.running {
            return Err(AppError::new(
                "IN_PROGRESS",
                "이미 가져오는 중입니다. 끝날 때까지 기다려 주세요.",
            ));
        }
        s.running = true;
        Ok(Session {
            id: s.id.clone(),
            file_name: s.file_name.clone(),
            sheet_name: s.sheet_name.clone(),
            school_year: s.school_year,
            mapping: s.mapping.clone(),
            rows: s.rows.clone(),
            running: true,
        })
    }

    /// 저장이 끝났다. 성공했으면 자리를 비운다 (같은 파일을 두 번 넣지 못하게).
    pub fn finish(&self, keep: bool) {
        let mut g = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if keep {
            if let Some(s) = g.as_mut() {
                s.running = false;
            }
        } else {
            *g = None;
        }
    }
}

/// 화면에 보여 줄 항목 설명. 목록의 원본은 Rust 한 곳에만 둔다.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldInfo {
    pub key: mapping::Field,
    pub label: String,
    pub required: bool,
}

pub fn field_list() -> Vec<FieldInfo> {
    mapping::Field::ALL
        .iter()
        .map(|f| FieldInfo {
            key: *f,
            label: f.label().to_string(),
            required: f.required(),
        })
        .collect()
}
