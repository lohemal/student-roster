//! 진급 배정 파일 읽기.
//!
//! 학교가 확정한 새 학급 편성을 받아 오는 길이다. 프로그램이 만든 양식을 사람이
//! 엑셀에서 채워 다시 넣으므로, **사람이 열을 옮기거나 지웠을 수 있다고 보고** 읽는다.
//!
//! 읽기만 한다 — 이 파일이 맞는지, 누가 빠졌는지는 `transition::build` 가 DB 와
//! 견주어 판단한다. 파일에 적힌 학생 번호도 그대로 믿지 않는다.

use crate::domain::transition::Col;
use crate::error::{AppError, AppResult};
use crate::import::excel;

/// 사람이 열 이름을 조금 다르게 적어도 알아본다.
fn aliases(c: Col) -> &'static [&'static str] {
    match c {
        Col::SchoolYear => &["학년도", "기존학년도", "원본학년도"],
        Col::StudentId => &["학생번호", "학생id", "studentid", "id"],
        Col::OldGrade => &["기존학년", "현재학년", "이전학년", "올해학년"],
        Col::OldClass => &["기존반", "현재반", "이전반", "올해반"],
        Col::OldNo => &["기존번호", "현재번호", "이전번호", "올해번호"],
        Col::Name => &["이름", "성명", "학생명"],
        Col::Birth => &["생년월일", "생일"],
        Col::NewGrade => &["새학년", "다음학년", "내년학년", "배정학년"],
        Col::NewClass => &["새반", "다음반", "내년반", "배정반"],
        Col::NewNo => &["새번호", "다음번호", "내년번호", "배정번호"],
    }
}

/// 견주기용 — 공백·괄호 따위를 없애고 소문자로.
fn norm(v: &str) -> String {
    v.chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, '(' | ')' | '[' | ']' | '_' | '-' | '.'))
        .flat_map(char::to_lowercase)
        .collect()
}

/// 배정 파일 한 줄. 사람이 적은 값 그대로 담는다.
#[derive(Debug, Clone)]
pub struct AssignRow {
    /// 엑셀에서 보이는 줄 번호
    pub excel_row: usize,
    pub school_year: Option<i32>,
    pub student_id: Option<i64>,
    pub old_grade: Option<i32>,
    pub old_class: Option<String>,
    pub old_no: Option<i32>,
    pub name: String,
    pub birth: Option<String>,
    pub new_grade: Option<i32>,
    pub new_class: Option<String>,
    pub new_no: Option<i32>,
}

fn text(cells: &[String], idx: Option<usize>) -> Option<String> {
    idx.and_then(|i| cells.get(i))
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn number(cells: &[String], idx: Option<usize>) -> Option<i64> {
    let raw = text(cells, idx)?;
    // `3`, `3.0`, ` 3 ` 을 모두 같게 읽는다
    let digits: String = raw
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    digits
        .split('.')
        .next()
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse().ok())
}

/// 읽은 결과.
#[derive(Debug)]
pub struct ReadResult {
    pub sheet_name: String,
    /// 머리글이 있던 줄 (사람이 보는 번호)
    pub header_row: usize,
    pub rows: Vec<AssignRow>,
    /// 찾지 못한 열 (없어도 되는 것도 있다)
    pub missing: Vec<&'static str>,
}

/// 파일을 읽어 배정 줄로 만든다.
///
/// 시트를 고르지 않으면 **머리글을 가진 첫 시트**를 쓴다. 양식을 그대로 쓰는 경우가
/// 대부분이라 사람에게 시트를 한 번 더 묻지 않는다.
pub fn read(path: &str, sheet: Option<&str>) -> AppResult<ReadResult> {
    let info = excel::inspect(path)?;
    let names: Vec<String> = match sheet {
        Some(s) => vec![s.to_string()],
        None => info.sheets.iter().map(|s| s.name.clone()).collect(),
    };

    let mut last_err: Option<AppError> = None;
    for name in names {
        let all = match excel::read_sheet(path, &name) {
            Ok(v) => v,
            Err(e) => {
                last_err = Some(e);
                continue;
            }
        };
        match parse_sheet(&name, &all) {
            Ok(r) => return Ok(r),
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.unwrap_or_else(|| {
        AppError::invalid("배정 자료를 읽지 못했습니다. 파일을 다시 골라 주세요.")
    }))
}

fn parse_sheet(sheet_name: &str, all: &[(usize, Vec<String>)]) -> AppResult<ReadResult> {
    // 머리글 줄 찾기 — 이름과 새 반이 함께 있는 줄이다
    let mut found: Option<(usize, usize, Vec<Option<usize>>)> = None;
    for (i, (excel_row, cells)) in all.iter().enumerate().take(30) {
        let picked = match_headers(cells);
        let has_name = picked[index_of(Col::Name)].is_some();
        let has_new_class = picked[index_of(Col::NewClass)].is_some();
        if has_name && has_new_class {
            found = Some((i, *excel_row, picked));
            break;
        }
    }
    let (at, header_row, cols) = found.ok_or_else(|| {
        AppError::invalid(
            "'이름' 과 '새 반' 열을 찾지 못했습니다. [진급 배정 양식 받기] 로 만든 파일인지 확인해 주세요.",
        )
    })?;

    let missing: Vec<&'static str> = Col::ALL
        .iter()
        .filter(|c| cols[index_of(**c)].is_none())
        .map(|c| c.header())
        .collect();

    let mut rows = Vec::new();
    for (excel_row, cells) in all.iter().skip(at + 1) {
        let name = text(cells, cols[index_of(Col::Name)]).unwrap_or_default();
        let has_any = Col::ALL
            .iter()
            .any(|c| text(cells, cols[index_of(*c)]).is_some());
        if name.is_empty() && !has_any {
            continue; // 빈 줄
        }
        rows.push(AssignRow {
            excel_row: *excel_row,
            school_year: number(cells, cols[index_of(Col::SchoolYear)]).map(|v| v as i32),
            student_id: number(cells, cols[index_of(Col::StudentId)]),
            old_grade: number(cells, cols[index_of(Col::OldGrade)]).map(|v| v as i32),
            old_class: text(cells, cols[index_of(Col::OldClass)]),
            old_no: number(cells, cols[index_of(Col::OldNo)]).map(|v| v as i32),
            name,
            birth: text(cells, cols[index_of(Col::Birth)]),
            new_grade: number(cells, cols[index_of(Col::NewGrade)]).map(|v| v as i32),
            new_class: text(cells, cols[index_of(Col::NewClass)]),
            new_no: number(cells, cols[index_of(Col::NewNo)]).map(|v| v as i32),
        });
    }

    if rows.is_empty() {
        return Err(AppError::invalid(
            "머리글 아래에 자료가 없습니다. 파일을 확인해 주세요.",
        ));
    }

    Ok(ReadResult {
        sheet_name: sheet_name.to_string(),
        header_row,
        rows,
        missing,
    })
}

fn index_of(c: Col) -> usize {
    Col::ALL.iter().position(|x| *x == c).expect("ALL 에 있다")
}

/// 머리글 줄에서 각 열이 몇 번째 칸인지 찾는다.
fn match_headers(cells: &[String]) -> Vec<Option<usize>> {
    let normed: Vec<String> = cells.iter().map(|v| norm(v)).collect();
    Col::ALL
        .iter()
        .map(|c| {
            let aliases = aliases(*c);
            normed
                .iter()
                .position(|v| aliases.iter().any(|a| v == a))
        })
        .collect()
}
