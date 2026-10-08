//! 통계 Excel.
//!
//! **화면에 보이는 숫자를 그대로 옮긴다.** 집계를 여기서 다시 하지 않는다 —
//! `repo::stats` 가 센 결과를 받아 표 모양으로만 바꾼다. 세는 규칙이 두 군데 있으면
//! 언젠가 화면과 파일이 다른 숫자를 말하고, 그러면 둘 다 못 쓴다.
//!
//! 고른 통계마다 시트 한 장, 전체는 파일 **하나**다. 통계는 셋을 나란히 놓고 견주는
//! 자료이므로 파일을 셋으로 나누면 쓰기 불편하다.
//!
//! 열은 화면과 같다. 화면에 있는 합계 줄은 넣고, 화면에 없는 합계는 만들지 않는다.

use serde::Deserialize;

use crate::repo::stats::{AddressTable, ClassCount, Counts, GradeRow};

use super::{ExportPlan, FilePlan, SheetPlan};

/// 고를 수 있는 통계 한 가지.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Kind {
    /// 학년별 현황
    Grade,
    /// 학년·반별 현황
    Class,
    /// 주소 분류 현황
    Address,
}

impl Kind {
    pub const ALL: [Kind; 3] = [Kind::Grade, Kind::Class, Kind::Address];

    /// 화면이 돌려보내는 값
    pub fn code(self) -> &'static str {
        match self {
            Kind::Grade => "GRADE",
            Kind::Class => "CLASS",
            Kind::Address => "ADDRESS",
        }
    }

    /// 화면에서 고를 때 보이는 이름
    pub fn label(self) -> &'static str {
        match self {
            Kind::Grade => "학년별 현황",
            Kind::Class => "학년 · 반별 현황",
            Kind::Address => "주소 분류 현황",
        }
    }

    /// 시트 이름. Excel 이 받는 글자만 쓴다(`·` 는 쓸 수 있지만 짧게 둔다).
    pub fn sheet_name(self) -> &'static str {
        match self {
            Kind::Grade => "학년별",
            Kind::Class => "학년반별",
            Kind::Address => "주소분류",
        }
    }
}

/// 시트에 담을 값. 화면이 센 것을 그대로 받는다.
pub struct Source<'a> {
    pub school_year: i32,
    pub totals: &'a Counts,
    pub by_grade: &'a [GradeRow],
    pub by_class: &'a [ClassCount],
    pub address: &'a AddressTable,
}

/// `{학년도}학년도_학생현황통계`
pub fn file_name(school_year: i32) -> String {
    format!("{school_year}학년도_학생현황통계")
}

fn num(v: i64) -> String {
    v.to_string()
}

/// 남 · 여 · 미입력 · 합계 — 학년별과 학년·반별이 함께 쓴다(화면과 같은 차례).
fn count_cells(c: &Counts) -> Vec<String> {
    vec![num(c.male), num(c.female), num(c.unknown), num(c.total)]
}

fn grade_sheet(src: &Source<'_>) -> SheetPlan {
    let mut rows: Vec<Vec<String>> = src
        .by_grade
        .iter()
        .map(|r| {
            let mut row = vec![format!("{}학년", r.grade)];
            row.extend(count_cells(&r.counts));
            row
        })
        .collect();
    // 화면 맨 아랫줄과 같다
    let mut total = vec!["합계".to_string()];
    total.extend(count_cells(src.totals));
    rows.push(total);

    SheetPlan {
        name: Kind::Grade.sheet_name().into(),
        headers: vec!["학년".into(), "남".into(), "여".into(), "미입력".into(), "합계".into()],
        widths: vec![12.0, 8.0, 8.0, 10.0, 10.0],
        numeric: vec![false, true, true, true, true],
        rows,
    }
}

fn class_sheet(src: &Source<'_>) -> SheetPlan {
    // 화면은 `3-나리` 한 칸이지만 파일에서는 학년과 반을 나눈다 — 거르고 합치는
    // 일을 Excel 에서 하게 된다. 세는 대상과 숫자는 그대로다.
    let mut rows: Vec<Vec<String>> = src
        .by_class
        .iter()
        .map(|r| {
            let class = match r.class_name.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                Some(c) => c.to_string(),
                None => "미정".to_string(),
            };
            let mut row = vec![format!("{}학년", r.grade), class];
            row.extend(count_cells(&r.counts));
            row
        })
        .collect();
    let mut total = vec!["합계".to_string(), String::new()];
    total.extend(count_cells(src.totals));
    rows.push(total);

    SheetPlan {
        name: Kind::Class.sheet_name().into(),
        headers: vec![
            "학년".into(),
            "반".into(),
            "남".into(),
            "여".into(),
            "미입력".into(),
            "합계".into(),
        ],
        widths: vec![10.0, 12.0, 8.0, 8.0, 10.0, 10.0],
        numeric: vec![false, false, true, true, true, true],
        rows,
    }
}

fn address_sheet(src: &Source<'_>) -> SheetPlan {
    let t = src.address;
    let mut headers = vec!["주소 분류".to_string()];
    headers.extend(t.grades.iter().map(|g| format!("{g}학년")));
    headers.push("합계".into());

    let mut widths = vec![18.0];
    widths.extend(t.grades.iter().map(|_| 9.0));
    widths.push(10.0);

    let mut numeric = vec![false];
    numeric.extend(t.grades.iter().map(|_| true));
    numeric.push(true);

    let mut rows: Vec<Vec<String>> = t
        .rows
        .iter()
        .map(|r| {
            let mut row = vec![r.name.clone()];
            // 열 수를 머리글과 맞춘다 — 빈 칸은 0 이다(화면과 같다)
            for i in 0..t.grades.len() {
                row.push(num(r.by_grade.get(i).copied().unwrap_or(0)));
            }
            row.push(num(r.total));
            row
        })
        .collect();

    let mut total = vec!["합계".to_string()];
    for i in 0..t.grades.len() {
        total.push(num(t.grade_totals.get(i).copied().unwrap_or(0)));
    }
    total.push(num(t.total));
    rows.push(total);

    SheetPlan {
        name: Kind::Address.sheet_name().into(),
        headers,
        widths,
        numeric,
        rows,
    }
}

/// 고른 통계로 파일 하나를 만든다.
///
/// `picked` 의 차례는 보지 않는다 — 시트 차례는 화면 차례(학년별 → 학년·반별 →
/// 주소 분류)로 언제나 같다. 고른 차례대로 놓으면 같은 파일이 열 때마다 달라진다.
pub fn plan(src: &Source<'_>, picked: &[Kind]) -> ExportPlan {
    let sheets: Vec<SheetPlan> = Kind::ALL
        .into_iter()
        .filter(|k| picked.contains(k))
        .map(|k| match k {
            Kind::Grade => grade_sheet(src),
            Kind::Class => class_sheet(src),
            Kind::Address => address_sheet(src),
        })
        .collect();

    let students = src.totals.total as usize;
    ExportPlan {
        files: vec![FilePlan {
            file_name: file_name(src.school_year),
            label: format!("{}학년도 학생현황통계", src.school_year),
            sheets,
        }],
        students,
        matched: students,
        warnings: Vec::new(),
    }
}

#[cfg(test)]
#[path = "stats_tests.rs"]
mod stats_tests;
