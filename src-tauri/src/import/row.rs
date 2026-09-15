//! 엑셀 한 줄에서 학생 값 꺼내기.
//!
//! 여기서는 **읽기만** 한다. 저장 규칙(정규화·확인 필요 판정)은 손대지 않고
//! `domain` 과 `repo::student` 가 하던 그대로 쓴다.

use chrono::NaiveDate;
use serde::Serialize;

use super::excel::restore_phone_leading_zero;
use super::mapping::{Field, Mapping};
use crate::domain::{birth, korean};
use crate::repo::student::StudentInput;

/// 엑셀 한 줄에서 꺼낸 값. 글자는 원본 그대로 두고, 숫자로 읽은 것만 따로 담는다.
#[derive(Debug, Clone, Default)]
pub struct ParsedRow {
    /// 사람이 엑셀에서 보는 줄 번호 (1부터)
    pub excel_row: usize,
    pub name: String,
    pub grade: Option<i32>,
    pub class_name: Option<String>,
    pub class_no: Option<i32>,
    pub gender: Option<String>,
    pub birth_raw: Option<String>,
    pub birth_date: Option<NaiveDate>,
    /// 생년월일을 읽으며 생긴 문제 (있어도 등록은 막지 않는다)
    pub birth_problem: Option<String>,
    pub address: Option<String>,
    pub father_name: Option<String>,
    pub mother_name: Option<String>,
    pub father_phone: Option<String>,
    pub mother_phone: Option<String>,
    pub primary_phone: Option<String>,
    pub note: Option<String>,
    /// 학년을 알 수 없는 등 이 줄을 쓸 수 없는 이유
    pub blocker: Option<String>,
}

fn take(cells: &[String], mapping: &Mapping, field: Field) -> Option<String> {
    let col = *mapping.get(&field)?;
    let v = cells.get(col)?.trim();
    if v.is_empty() {
        None
    } else {
        Some(v.to_string())
    }
}

/// `3`, `3학년`, `3 학년` 에서 숫자만 뽑는다.
fn parse_int(v: &str) -> Option<i32> {
    let digits: String = v.chars().filter(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// `남`/`여`, `M`/`F`, `1`/`2` 를 모두 알아본다.
fn parse_gender(v: &str) -> Option<String> {
    let t = v.trim();
    let first = t.chars().next()?;
    match first {
        '남' | 'M' | 'm' | '1' => Some("M".to_string()),
        '여' | 'F' | 'f' | '2' => Some("F".to_string()),
        _ => None,
    }
}

/// 엑셀 한 줄을 읽는다. `default_grade` 는 학년 열이 없을 때만 쓴다.
pub fn parse(
    excel_row: usize,
    cells: &[String],
    mapping: &Mapping,
    default_grade: Option<i32>,
    today: NaiveDate,
) -> ParsedRow {
    let name = take(cells, mapping, Field::Name).unwrap_or_default();

    let grade = take(cells, mapping, Field::Grade)
        .and_then(|v| parse_int(&v))
        .or(default_grade);

    let birth_raw = take(cells, mapping, Field::Birth);
    let parsed = birth::parse(birth_raw.as_deref().unwrap_or(""), today);

    let phone = |field: Field| {
        take(cells, mapping, field).map(|v| restore_phone_leading_zero(&v))
    };

    let mut row = ParsedRow {
        excel_row,
        grade,
        class_name: take(cells, mapping, Field::ClassName),
        class_no: take(cells, mapping, Field::ClassNo).and_then(|v| parse_int(&v)),
        gender: take(cells, mapping, Field::Gender).and_then(|v| parse_gender(&v)),
        birth_date: parsed.date,
        birth_problem: parsed.problem.as_ref().map(|p| p.message()),
        birth_raw,
        address: take(cells, mapping, Field::Address),
        father_name: take(cells, mapping, Field::FatherName),
        mother_name: take(cells, mapping, Field::MotherName),
        father_phone: phone(Field::FatherPhone),
        mother_phone: phone(Field::MotherPhone),
        primary_phone: phone(Field::PrimaryPhone),
        note: take(cells, mapping, Field::Note),
        name,
        blocker: None,
    };

    // 이 줄을 아예 쓸 수 없는 경우만 막는다. 나머지 빈칸은 등록한 뒤 확인 필요로 남긴다.
    if row.name.is_empty() {
        row.blocker = Some("이름이 비어 있어 가져올 수 없습니다.".into());
    } else if row.name.chars().count() > 30 {
        row.blocker = Some("이름이 30자를 넘어 가져올 수 없습니다.".into());
    } else if row.grade.is_none() {
        row.blocker = Some("학년을 알 수 없어 가져올 수 없습니다.".into());
    } else if !(1..=6).contains(&row.grade.unwrap_or(0)) {
        row.blocker = Some(format!(
            "학년이 {}입니다. 1~6학년만 가져올 수 있습니다.",
            row.grade.unwrap_or(0)
        ));
    } else if row.class_no.map(|n| !(1..=200).contains(&n)).unwrap_or(false) {
        row.blocker = Some("번호가 1~200을 벗어나 가져올 수 없습니다.".into());
    }

    row
}

/// 새 학생으로 등록할 때 넘길 값.
pub fn to_input(row: &ParsedRow, school_year: i32) -> StudentInput {
    StudentInput {
        name: row.name.clone(),
        gender: row.gender.clone(),
        birth_raw: row.birth_raw.clone(),
        address_raw: row.address.clone(),
        father_name: row.father_name.clone(),
        mother_name: row.mother_name.clone(),
        father_phone: row.father_phone.clone(),
        mother_phone: row.mother_phone.clone(),
        primary_phone: row.primary_phone.clone(),
        note: row.note.clone(),
        school_year,
        grade: row.grade.unwrap_or(1),
        class_name: row.class_name.clone(),
        class_no: row.class_no,
    }
}

/// 이 줄에서 **미리 알 수 있는** 확인거리. 저장 뒤 실제 판정은 `sync_issues` 가 한다.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RowWarning {
    pub message: String,
}

pub fn warnings(row: &ParsedRow) -> Vec<RowWarning> {
    let mut out = Vec::new();
    if let Some(p) = &row.birth_problem {
        out.push(RowWarning { message: p.clone() });
    }
    let mut empty: Vec<&str> = Vec::new();
    if row.gender.is_none() {
        empty.push("성별");
    }
    if row.birth_raw.is_none() {
        empty.push("생년월일");
    }
    if row.class_name.is_none() {
        empty.push("반");
    }
    if row.class_no.is_none() {
        empty.push("번호");
    }
    if row.address.is_none() {
        empty.push("주소");
    }
    if row.father_phone.is_none() && row.mother_phone.is_none() && row.primary_phone.is_none() {
        empty.push("보호자 연락처");
    }
    if !empty.is_empty() {
        out.push(RowWarning {
            message: korean::list_with_subject(&empty, "비어 있습니다."),
        });
    }
    out
}

#[cfg(test)]
#[path = "row_tests.rs"]
mod row_tests;
