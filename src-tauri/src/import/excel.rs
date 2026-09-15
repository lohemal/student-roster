//! Excel 파일 읽기.
//!
//! calamine 이 돌려주는 칸 값은 종류가 여럿이라(글자·숫자·날짜·오류) 그대로 쓸 수 없다.
//! 여기서 **모두 글자로 바꿔** 놓고, 뜻을 읽는 일은 `domain` 에 맡긴다.
//!
//! 엑셀에서 자주 겪는 두 가지를 여기서 처리한다.
//!   * 숫자 칸으로 저장된 값 — `170315` 가 `170315.0` 으로 읽히면 안 된다.
//!   * 연락처의 사라진 앞자리 0 — `010-1234-5678` 을 숫자로 넣으면 `1012345678` 이 된다.

use calamine::{open_workbook_auto, Data, Range, Reader};
use serde::Serialize;

use crate::error::{AppError, AppResult};

/// 미리보기로 보여 줄 줄 수.
pub const PREVIEW_ROWS: usize = 8;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetInfo {
    pub name: String,
    pub rows: usize,
    pub cols: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
    pub file_name: String,
    pub sheets: Vec<SheetInfo>,
}

/// 시트 목록과 크기만 먼저 본다.
pub fn inspect(path: &str) -> AppResult<FileInfo> {
    let mut wb = open(path)?;
    let names = wb.sheet_names().to_vec();

    let mut sheets = Vec::new();
    for name in names {
        let (rows, cols) = match wb.worksheet_range(&name) {
            Ok(r) => r.get_size(),
            // 차트 시트처럼 표가 아닌 시트는 크기를 0으로 두고 목록에는 남긴다
            Err(_) => (0, 0),
        };
        sheets.push(SheetInfo { name, rows, cols });
    }

    if sheets.is_empty() {
        return Err(AppError::invalid(
            "이 파일에는 시트가 없습니다. 다른 파일을 골라 주세요.",
        ));
    }

    Ok(FileInfo {
        file_name: file_name_of(path),
        sheets,
    })
}

/// 시트 하나를 통째로 글자표로 읽는다. 빈 줄은 버린다.
///
/// 돌려주는 값은 `(원본 줄 번호, 칸 값들)` 이다. 줄 번호는 사람이 엑셀에서 보는
/// 1부터 시작하는 번호라, 문제가 있는 줄을 알려 줄 때 그대로 쓸 수 있다.
pub fn read_sheet(path: &str, sheet: &str) -> AppResult<Vec<(usize, Vec<String>)>> {
    let mut wb = open(path)?;
    let range: Range<Data> = wb.worksheet_range(sheet).map_err(|e| {
        AppError::invalid(format!(
            "'{sheet}' 시트를 읽지 못했습니다. 시트를 다시 골라 주세요."
        ))
        .detail(e.to_string())
    })?;

    // 표가 시트 한가운데에서 시작할 수 있다. 그만큼 줄 번호를 밀어 준다.
    let first_row = range.start().map(|(r, _)| r as usize).unwrap_or(0);

    let mut out = Vec::with_capacity(range.height());
    for (i, row) in range.rows().enumerate() {
        let cells: Vec<String> = row.iter().map(cell_to_string).collect();
        if cells.iter().all(|c| c.is_empty()) {
            continue; // 빈 줄은 세지 않는다
        }
        out.push((first_row + i + 1, cells));
    }
    Ok(out)
}

fn open(path: &str) -> AppResult<calamine::Sheets<std::io::BufReader<std::fs::File>>> {
    if !std::path::Path::new(path).exists() {
        return Err(AppError::not_found(
            "파일을 찾을 수 없습니다. 파일이 옮겨졌거나 지워졌는지 확인해 주세요.",
        ));
    }
    open_workbook_auto(path).map_err(|e| {
        AppError::invalid(
            "엑셀 파일을 열지 못했습니다. .xlsx 또는 .xls 파일인지, 다른 프로그램에서 열어 두지 않았는지 확인해 주세요.",
        )
        .detail(e.to_string())
    })
}

fn file_name_of(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

/// 칸 하나를 글자로. 뜻은 풀지 않고 **보이는 대로** 옮긴다.
pub fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(s) => s.trim().to_string(),
        Data::Int(n) => n.to_string(),
        Data::Float(f) => float_to_string(*f),
        Data::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
        // 날짜 칸은 YYYY-MM-DD 로 — domain::birth 가 이 모양을 읽는다
        Data::DateTime(dt) => match dt.as_datetime() {
            Some(d) => d.date().format("%Y-%m-%d").to_string(),
            None => float_to_string(dt.as_f64()),
        },
        Data::DateTimeIso(s) => s.split('T').next().unwrap_or(s).to_string(),
        Data::DurationIso(s) => s.clone(),
        // #N/A 같은 오류 칸은 빈 값으로 본다
        Data::Error(_) => String::new(),
    }
}

/// `170315.0` 이 아니라 `170315` 가 되도록.
fn float_to_string(f: f64) -> String {
    if f.is_finite() && f.fract() == 0.0 && f.abs() < 1e15 {
        format!("{}", f as i64)
    } else {
        // 부동소수 찌꺼기(0.30000000000000004)를 남기지 않는다
        let s = format!("{f}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// 연락처 칸의 사라진 앞자리 0을 되살린다.
///
/// 엑셀에서 `010-1234-5678` 을 숫자로 넣으면 `1012345678` 이 되어 앞의 0이 없어진다.
/// 숫자만 있고 자릿수가 9~10자리이며 1이나 2로 시작하면 0을 붙인다.
/// (`01012345678` 은 11자리라 그대로, `0441234567` 은 0으로 시작하므로 그대로.)
pub fn restore_phone_leading_zero(raw: &str) -> String {
    let t = raw.trim();
    if t.is_empty() || !t.chars().all(|c| c.is_ascii_digit()) {
        return t.to_string();
    }
    let starts_ok = t.starts_with('1') || t.starts_with('2');
    if (t.len() == 9 || t.len() == 10) && starts_ok {
        format!("0{t}")
    } else {
        t.to_string()
    }
}

#[cfg(test)]
#[path = "excel_tests.rs"]
mod excel_tests;
