//! 내보내기 규칙 — 어떤 열을, 어떤 파일로 나눠, 어떤 이름으로.
//!
//! 사용자 지정 Excel·학교종이·알림e 는 **파일 쓰는 코드를 함께 쓴다.** 다른 것은
//! 열 정의와 묶는 방법뿐이므로 그 규칙만 여기 모아 둔다. 양식이 바뀌면 여기만 고친다.
//!
//! 지키는 것
//!   * 학생 자료를 **임의로 고쳐 내보내지 않는다.** 주소는 원본을 그대로 쓰고,
//!     이름이 길다고 잘라 내지 않는다. 시스템이 만든 값(비고 등)만 규격에 맞춘다.
//!   * 전화번호는 **글자**로 쓴다. 숫자로 읽히면 앞의 0 이 사라진다.
//!   * 어떤 학생을 뽑을지(`필터`)와 **몇 개 파일로 나눌지(`묶기`)는 다른 이야기**다.
//!     3학년만 뽑아 반별로 나눌 수도 있고, 5단지만 뽑아 학년별로 나눌 수도 있다.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------
// 열
// ---------------------------------------------------------------

/// 사용자 지정 Excel 에 넣을 수 있는 열.
///
/// 화면과 Rust 가 열 이름·차례를 따로 적어 두면 언젠가 어긋난다. 목록의 원본은
/// 여기 하나이고 화면은 `Column::ALL` 을 받아 그린다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Column {
    SchoolYear,
    Grade,
    ClassName,
    ClassNo,
    Name,
    Gender,
    BirthDate,
    Address,
    AddressCategory,
    FatherName,
    MotherName,
    FatherPhone,
    MotherPhone,
    PrimaryPhone,
    Sibling,
    Note,
}

impl Column {
    pub const ALL: [Column; 16] = [
        Column::SchoolYear,
        Column::Grade,
        Column::ClassName,
        Column::ClassNo,
        Column::Name,
        Column::Gender,
        Column::BirthDate,
        Column::Address,
        Column::AddressCategory,
        Column::FatherName,
        Column::MotherName,
        Column::FatherPhone,
        Column::MotherPhone,
        Column::PrimaryPhone,
        Column::Sibling,
        Column::Note,
    ];

    /// 처음 열었을 때의 기본 구성 — 학교에서 가장 흔히 쓰는 다섯 가지.
    pub const DEFAULT: [Column; 5] = [
        Column::Grade,
        Column::ClassName,
        Column::ClassNo,
        Column::Name,
        Column::Gender,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Column::SchoolYear => "SCHOOL_YEAR",
            Column::Grade => "GRADE",
            Column::ClassName => "CLASS_NAME",
            Column::ClassNo => "CLASS_NO",
            Column::Name => "NAME",
            Column::Gender => "GENDER",
            Column::BirthDate => "BIRTH_DATE",
            Column::Address => "ADDRESS",
            Column::AddressCategory => "ADDRESS_CATEGORY",
            Column::FatherName => "FATHER_NAME",
            Column::MotherName => "MOTHER_NAME",
            Column::FatherPhone => "FATHER_PHONE",
            Column::MotherPhone => "MOTHER_PHONE",
            Column::PrimaryPhone => "PRIMARY_PHONE",
            Column::Sibling => "SIBLING",
            Column::Note => "NOTE",
        }
    }

    pub fn parse(key: &str) -> Option<Self> {
        Column::ALL.into_iter().find(|c| c.key() == key)
    }

    /// 화면에 보여 줄 이름이자 Excel 머리글. 둘을 따로 두지 않는다.
    pub fn label(self) -> &'static str {
        match self {
            Column::SchoolYear => "학년도",
            Column::Grade => "학년",
            Column::ClassName => "반",
            Column::ClassNo => "번호",
            Column::Name => "이름",
            Column::Gender => "성별",
            Column::BirthDate => "생년월일",
            Column::Address => "주소",
            Column::AddressCategory => "주소 분류",
            Column::FatherName => "부 성명",
            Column::MotherName => "모 성명",
            Column::FatherPhone => "부 연락처",
            Column::MotherPhone => "모 연락처",
            Column::PrimaryPhone => "주보호자 연락처",
            Column::Sibling => "본교 형제",
            Column::Note => "비고",
        }
    }

    /// 개인정보에 해당하는 열인가. 화면에서 사용자에게 알려 주는 데 쓴다.
    pub fn is_personal(self) -> bool {
        !matches!(
            self,
            Column::SchoolYear | Column::Grade | Column::ClassName | Column::ClassNo
        )
    }

    /// 보기 좋은 열 너비 (Excel 문자 수 기준).
    pub fn width(self) -> f64 {
        match self {
            Column::Address => 38.0,
            Column::Sibling => 22.0,
            Column::Note => 20.0,
            Column::AddressCategory | Column::PrimaryPhone => 15.0,
            Column::FatherPhone | Column::MotherPhone | Column::BirthDate => 13.0,
            Column::Name | Column::FatherName | Column::MotherName => 10.0,
            Column::SchoolYear => 9.0,
            Column::ClassName => 8.0,
            _ => 6.0,
        }
    }
}

// ---------------------------------------------------------------
// 묶기
// ---------------------------------------------------------------

/// 뽑은 학생을 **몇 개 파일로 나눌지**.
///
/// 어떤 학생을 뽑을지(필터)와는 다른 이야기다. 3학년만 뽑아 반별로 나눌 수도 있고,
/// 5단지만 뽑아 학년별로 나눌 수도 있다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Grouping {
    /// 한 덩어리로
    All,
    /// 학년마다 따로
    Grade,
    /// 학년·반마다 따로
    GradeClass,
}

impl Grouping {
    pub fn label(self) -> &'static str {
        match self {
            Grouping::All => "전체",
            Grouping::Grade => "학년별",
            Grouping::GradeClass => "학년·반별",
        }
    }
}

/// 알림e 가 한 파일에 담을 수 있는 최대 인원.
pub const ALIME_MAX_ROWS: usize = 1000;

/// 알림e 각 칸의 글자 수 제한.
pub const ALIME_MAX_LEN: usize = 15;

// ---------------------------------------------------------------
// 이름 안전하게 만들기
// ---------------------------------------------------------------

/// Excel 시트 이름으로 쓸 수 없는 글자.
const SHEET_BAD: [char; 7] = ['[', ']', ':', '*', '?', '/', '\\'];

/// 시트 이름을 안전하게 만든다.
///
/// Excel 은 31자를 넘기거나 `[]:*?/\` 가 들어 있으면 파일을 열지 못한다.
/// 같은 이름이 두 번 나오면 뒤에 번호를 붙인다.
pub fn safe_sheet_name(raw: &str, used: &mut Vec<String>) -> String {
    let cleaned: String = raw
        .chars()
        .map(|c| if SHEET_BAD.contains(&c) { ' ' } else { c })
        .collect();
    let mut name = trim_chars(cleaned.trim(), 31);
    if name.is_empty() {
        name = "시트".to_string();
    }

    if used.iter().any(|u| u == &name) {
        for i in 2..1000 {
            let suffix = format!(" ({i})");
            let base = trim_chars(&name, 31 - suffix.chars().count());
            let candidate = format!("{base}{suffix}");
            if !used.iter().any(|u| u == &candidate) {
                name = candidate;
                break;
            }
        }
    }
    used.push(name.clone());
    name
}

/// 파일 이름으로 쓸 수 없는 글자.
const FILE_BAD: [char; 9] = ['\\', '/', ':', '*', '?', '"', '<', '>', '|'];

/// 파일 이름을 안전하게 만든다. 확장자는 붙이지 않는다.
///
/// 학교 이름이나 반 이름에 `/` 같은 글자가 들어 있어도 저장이 실패하지 않아야 한다.
pub fn safe_file_name(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .filter(|c| !c.is_control())
        .map(|c| if FILE_BAD.contains(&c) { '_' } else { c })
        .collect();
    // 윈도우는 이름 끝의 점·공백을 싫어한다
    let trimmed = cleaned.trim().trim_end_matches('.').trim();
    let out = trim_chars(trimmed, 120);
    if out.is_empty() {
        "명단".to_string()
    } else {
        out
    }
}

fn trim_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

// ---------------------------------------------------------------
// 값 다루기
// ---------------------------------------------------------------

/// 알림e 비고 — `3-나리-7`. 반이 없으면 `3학년`, 번호가 없으면 `3-나리`.
///
/// 15자를 넘지 않게 만든다. **학생 이름은 넣지 않는다** — 이름 칸에 이미 있다.
pub fn alime_note(grade: i32, class_name: Option<&str>, class_no: Option<i32>) -> String {
    let class_name = class_name.map(str::trim).filter(|s| !s.is_empty());
    let full = match (class_name, class_no) {
        (Some(c), Some(n)) => format!("{grade}-{c}-{n}"),
        (Some(c), None) => format!("{grade}-{c}"),
        (None, Some(n)) => format!("{grade}학년 {n}번"),
        (None, None) => format!("{grade}학년"),
    };
    if full.chars().count() <= ALIME_MAX_LEN {
        return full;
    }
    // 반 이름이 아주 길면 번호만이라도 남긴다
    let short = match class_no {
        Some(n) => format!("{grade}-{n}"),
        None => format!("{grade}학년"),
    };
    trim_chars(&short, ALIME_MAX_LEN)
}

/// 학교종이 시트 이름 — `1학년 가람반` / `3학년 2반`.
pub fn schooljongi_sheet(grade: i32, class_name: Option<&str>) -> String {
    match class_name.map(str::trim).filter(|s| !s.is_empty()) {
        Some(c) => format!("{grade}학년 {c}반"),
        None => format!("{grade}학년 반미정"),
    }
}

/// 보호자 연락처 두 칸을 고른다.
///
/// 학교종이의 `보호자휴대폰1` · `보호자휴대폰2` 에 넣을 값이다.
///   * 1번은 **주보호자 → 모 → 부** 차례로 있는 것을 쓴다.
///   * 2번은 1번에 쓰이지 않은 모·부 연락처 가운데 하나를 쓴다.
///   * **같은 번호를 1번과 2번에 두 번 적지 않는다.** 두 칸이 같으면 두 번 발송된다.
pub fn guardian_pair(
    primary: Option<&str>,
    mother: Option<&str>,
    father: Option<&str>,
) -> (Option<String>, Option<String>) {
    let clean = |v: Option<&str>| v.map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    let (primary, mother, father) = (clean(primary), clean(mother), clean(father));

    let first = primary.clone().or_else(|| mother.clone()).or_else(|| father.clone());
    let same = |a: &Option<String>, b: &Option<String>| match (a, b) {
        (Some(x), Some(y)) => digits(x) == digits(y),
        _ => false,
    };
    let second = [mother, father]
        .into_iter()
        .flatten()
        .find(|v| !same(&first, &Some(v.clone())));

    (first, second)
}

/// 견주기용 숫자만. `010-1234-5678` 과 `01012345678` 은 같은 번호다.
fn digits(v: &str) -> String {
    v.chars().filter(char::is_ascii_digit).collect()
}

/// 알림e 전화번호 — **주보호자 → 모 → 부** 차례로 있는 것을 쓴다.
pub fn alime_phone(
    primary: Option<&str>,
    mother: Option<&str>,
    father: Option<&str>,
) -> Option<String> {
    [primary, mother, father]
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|s| !s.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
#[path = "export_tests.rs"]
mod export_tests;
