//! Excel 내보내기.
//!
//! 사용자 지정·학교종이·알림e 가 **파일 쓰는 코드를 함께 쓴다.** 셋이 다른 것은
//! 열과 묶는 방법뿐이므로 그 부분만 `preset` 이 만들고, 쓰기는 이 모듈 하나가 한다.
//!
//! 흐름은 Phase 2 가져오기와 같은 모양이다 — **계획(`plan`) → 사람이 확인 → 저장(`write`)**.
//! 미리보기와 결과가 어긋날 수 없도록 화면이 보는 것과 파일이 되는 것이 같은 계획이다.
//!
//! 지키는 것
//!   * 모든 칸을 **글자로 쓴다.** `.xlsx` 의 글자 칸은 Excel 이 수식으로 계산하지 않고,
//!     `01012345678` 앞의 0 도 사라지지 않는다.
//!   * 학생 자료를 임의로 고치지 않는다. 주소는 원본 그대로, 이름은 자르지 않는다.
//!   * 로그에 학생 이름·연락처·주소를 남기지 않는다. 파일 수와 인원만 남긴다.

pub mod preset;

use std::path::{Path, PathBuf};

use rust_xlsxwriter::{Format, Workbook};
use serde::Serialize;

use crate::error::{AppError, AppResult};

/// 시트 한 장.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetPlan {
    pub name: String,
    pub headers: Vec<String>,
    /// 열 너비 (Excel 문자 수). 머리글 수와 같아야 한다.
    #[serde(skip)]
    pub widths: Vec<f64>,
    pub rows: Vec<Vec<String>>,
}

impl SheetPlan {
    pub fn students(&self) -> usize {
        self.rows.len()
    }
}

/// 파일 한 개.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilePlan {
    /// 확장자를 뺀 이름. 안전한 글자만 들어 있다.
    pub file_name: String,
    /// 화면에 보여 줄 이름 (`3학년 가람반` 등)
    pub label: String,
    pub sheets: Vec<SheetPlan>,
}

impl FilePlan {
    pub fn students(&self) -> usize {
        self.sheets.iter().map(SheetPlan::students).sum()
    }
}

/// 확인이 필요한 학생 하나.
///
/// 번호만 주면 화면이 "3명"이라고만 말할 수 있다. 누구인지 바로 알아보고
/// 기존 학생 창을 열어 고칠 수 있도록 이름표를 함께 준다.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    pub student_id: i64,
    /// `3-나리 홍길동`
    pub label: String,
}

/// 내보내기 전에 사용자가 확인할 것.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Warning {
    /// 무엇이 문제인지 한 줄
    pub message: String,
    /// 그 학생들
    pub students: Vec<Problem>,
    /// 이 학생들이 파일에서 빠지는가
    pub excluded: bool,
}

impl Warning {
    #[cfg(test)]
    pub fn ids(&self) -> Vec<i64> {
        self.students.iter().map(|p| p.student_id).collect()
    }
}

/// 만들어질 파일 전체.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportPlan {
    pub files: Vec<FilePlan>,
    /// 실제로 파일에 들어가는 학생 수
    pub students: usize,
    /// 조건에 든 학생 수 (빠진 학생을 포함한 전체)
    pub matched: usize,
    pub warnings: Vec<Warning>,
}

impl ExportPlan {
    pub fn is_single_file(&self) -> bool {
        self.files.len() == 1
    }
}

// ---------------------------------------------------------------
// 쓰기
// ---------------------------------------------------------------

/// 파일 하나를 만든다. `path` 는 `.xlsx` 까지 포함한 전체 경로다.
fn write_file(plan: &FilePlan, path: &Path) -> AppResult<()> {
    let mut book = Workbook::new();
    let head = Format::new().set_bold();

    for sheet in &plan.sheets {
        let ws = book.add_worksheet();
        ws.set_name(&sheet.name).map_err(xlsx_err)?;

        for (i, h) in sheet.headers.iter().enumerate() {
            let col = i as u16;
            ws.write_string_with_format(0, col, h, &head)
                .map_err(xlsx_err)?;
            if let Some(w) = sheet.widths.get(i) {
                ws.set_column_width(col, *w).map_err(xlsx_err)?;
            }
        }
        // 머리글 줄을 고정해 두면 1,000줄을 내려도 어느 열인지 보인다
        ws.set_freeze_panes(1, 0).map_err(xlsx_err)?;

        for (r, row) in sheet.rows.iter().enumerate() {
            for (i, v) in row.iter().enumerate() {
                // 언제나 글자 칸이다. 숫자로 쓰면 전화번호 앞의 0 이 사라지고,
                // 자동 판정에 맡기면 값이 날짜나 수식으로 바뀔 수 있다.
                ws.write_string(r as u32 + 1, i as u16, v)
                    .map_err(xlsx_err)?;
            }
        }
    }

    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    book.save(path).map_err(|e| save_err(path, e))?;
    Ok(())
}

/// 만들어질 파일 경로. **쓰기 전에** 같은 이름이 있는지 확인하는 데도 쓴다.
///
/// 파일이 하나면 `target` 을 그 파일 경로로, 여럿이면 폴더로 본다.
pub fn target_paths(plan: &ExportPlan, target: &Path) -> Vec<PathBuf> {
    if plan.is_single_file() {
        vec![target.to_path_buf()]
    } else {
        plan.files
            .iter()
            .map(|f| target.join(format!("{}.xlsx", f.file_name)))
            .collect()
    }
}

/// 계획대로 파일을 만든다.
///
/// 경로는 `target_paths` 가 정한다 — 미리 알려 준 곳과 실제로 만드는 곳이 갈라지지 않는다.
pub fn write_plan(plan: &ExportPlan, target: &Path) -> AppResult<Vec<PathBuf>> {
    if plan.files.is_empty() {
        return Err(AppError::invalid("내보낼 학생이 없습니다."));
    }

    let made = target_paths(plan, target);
    if !plan.is_single_file() {
        std::fs::create_dir_all(target)?;
    }
    for (f, path) in plan.files.iter().zip(made.iter()) {
        write_file(f, path)?;
    }
    // 개인정보는 남기지 않는다 — 몇 개 만들었는지만
    log::info!("export: {} file(s), {} student(s)", made.len(), plan.students);
    Ok(made)
}

fn xlsx_err(e: rust_xlsxwriter::XlsxError) -> AppError {
    AppError::new("EXPORT_FAILED", "엑셀 파일을 만들지 못했습니다.").detail(e.to_string())
}

/// 저장 실패는 까닭에 따라 할 일이 다르다. 사용자가 바로 고칠 수 있게 말해 준다.
fn save_err(path: &Path, e: rust_xlsxwriter::XlsxError) -> AppError {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "파일".into());
    let text = e.to_string();

    let message = if text.contains("Permission denied") || text.contains("os error 32") {
        format!("{name} 을(를) 저장하지 못했습니다. 같은 파일이 Excel 에서 열려 있으면 닫고 다시 시도해 주세요.")
    } else if text.contains("os error 5") || text.contains("Access is denied") {
        format!("{name} 을(를) 저장할 권한이 없습니다. 다른 폴더를 골라 주세요.")
    } else {
        format!("{name} 을(를) 저장하지 못했습니다.")
    };
    AppError::new("EXPORT_SAVE_FAILED", message).detail(text)
}

#[cfg(test)]
#[path = "export_tests.rs"]
mod export_tests;
