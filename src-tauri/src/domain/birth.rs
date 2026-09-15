//! 생년월일 정규화.
//!
//! 사람이 손으로 옮겨 적은 값이라 모양이 제각각이다.
//!
//! ```text
//! 170315   17.03.15   17.03.15.   17-03-15   20170315   2017.03.15   2017/3/5
//! ```
//!
//! 규칙
//!   * 숫자만 뽑아 6자리(YYMMDD) 또는 8자리(YYYYMMDD)일 때만 날짜로 본다.
//!   * 6자리의 세기는 **초등학생이라는 사실**로 정한다 — 20YY 로 읽어서 나이가
//!     맞으면 20YY, 아니면 19YY 를 시험해 본다. 둘 다 아니면 20YY 로 두되
//!     "확인 필요"를 붙인다. 있지도 않은 날짜를 만들어 내지 않는다.
//!   * 달력에 없는 날짜(2월 30일 등)는 **고치지 않고** 확인 필요로 남긴다.
//!   * 원본(`raw`)은 무슨 일이 있어도 그대로 보존한다.

use chrono::{Datelike, NaiveDate};

/// 초등학생으로 볼 수 있는 나이 범위 (만). 취학유예·조기입학까지 넉넉히 잡는다.
const AGE_MIN: i32 = 4;
const AGE_MAX: i32 = 15;

/// 생년월일을 읽은 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BirthParse {
    /// `YYYY-MM-DD`. 날짜로 읽지 못했으면 None.
    pub date: Option<NaiveDate>,
    /// 사용자에게 보여줄 확인 필요 사유. 문제가 없으면 None.
    pub problem: Option<BirthProblem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BirthProblem {
    /// 숫자 6자리·8자리가 아니라 날짜로 읽을 수 없다.
    Unreadable,
    /// 달력에 없는 날짜다 (2월 30일 등).
    NotACalendarDate,
    /// 날짜로는 읽히지만 초등학생 나이 범위를 벗어난다.
    OutOfRange { age: i32 },
}

impl BirthProblem {
    /// 화면에 그대로 보여줄 한국어 문장.
    pub fn message(&self) -> String {
        match self {
            BirthProblem::Unreadable => {
                "생년월일을 날짜로 읽지 못했습니다. 예: 170315 또는 2017.03.15".to_string()
            }
            BirthProblem::NotACalendarDate => {
                "달력에 없는 날짜입니다. 생년월일을 확인해 주세요.".to_string()
            }
            BirthProblem::OutOfRange { age } => format!(
                "생년월일로 계산한 나이가 만 {age}세입니다. 초등학생 범위를 벗어나므로 확인해 주세요."
            ),
        }
    }
}

impl BirthParse {
    fn ok(date: NaiveDate) -> Self {
        Self {
            date: Some(date),
            problem: None,
        }
    }
    fn flagged(date: Option<NaiveDate>, problem: BirthProblem) -> Self {
        Self {
            date,
            problem: Some(problem),
        }
    }
}

/// 빈 값인지 (공백만 있어도 빈 값으로 본다).
pub fn is_blank(raw: &str) -> bool {
    raw.trim().is_empty()
}

/// 생년월일 원문을 읽는다. `today`는 나이 계산 기준일.
///
/// 빈 값이면 `date: None, problem: None` — 비어 있는 것 자체는 문제로 보지 않는다
/// (필수값 확인은 따로 한다).
pub fn parse(raw: &str, today: NaiveDate) -> BirthParse {
    if is_blank(raw) {
        return BirthParse {
            date: None,
            problem: None,
        };
    }

    let Some((year, month, day)) = split_parts(raw, today).or_else(|| packed(raw, today)) else {
        return BirthParse::flagged(None, BirthProblem::Unreadable);
    };

    let Some(date) = NaiveDate::from_ymd_opt(year, month, day) else {
        return BirthParse::flagged(None, BirthProblem::NotACalendarDate);
    };

    let age = age_on(date, today);
    if !(AGE_MIN..=AGE_MAX).contains(&age) {
        // 날짜 자체는 읽혔으므로 값은 남기고 확인만 요청한다.
        return BirthParse::flagged(Some(date), BirthProblem::OutOfRange { age });
    }

    BirthParse::ok(date)
}

/// `2017.3.5` 처럼 구분자로 나뉜 값. 세 토막이고 연도가 2자리나 4자리일 때만 쓴다.
///
/// 이 길을 먼저 시도하는 이유: `2017/3/5` 의 숫자만 모으면 `201735` 라 여섯 자리가
/// 되어 20년 17월 35일로 잘못 읽힌다.
fn split_parts(raw: &str, today: NaiveDate) -> Option<(i32, u32, u32)> {
    let parts: Vec<&str> = raw
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .collect();

    if parts.len() != 3 {
        return None;
    }
    let (y, m, d) = (parts[0], parts[1], parts[2]);
    if m.len() > 2 || d.len() > 2 {
        return None;
    }

    let month = m.parse::<u32>().ok()?;
    let day = d.parse::<u32>().ok()?;

    match y.len() {
        4 => Some((y.parse::<i32>().ok()?, month, day)),
        2 => Some((
            guess_century(y.parse::<i32>().ok()?, month, day, today),
            month,
            day,
        )),
        _ => None,
    }
}

/// 구분자 없이 붙여 쓴 값. 6자리(YYMMDD) 또는 8자리(YYYYMMDD).
fn packed(raw: &str, today: NaiveDate) -> Option<(i32, u32, u32)> {
    let digits: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
    match digits.len() {
        8 => Some((
            digits[0..4].parse().ok()?,
            digits[4..6].parse().ok()?,
            digits[6..8].parse().ok()?,
        )),
        6 => {
            let yy = digits[0..2].parse::<i32>().ok()?;
            let month = digits[2..4].parse::<u32>().ok()?;
            let day = digits[4..6].parse::<u32>().ok()?;
            Some((guess_century(yy, month, day, today), month, day))
        }
        _ => None,
    }
}

/// 2자리 연도의 세기를 정한다.
///
/// 1. 20YY 로 읽어 초등학생 나이면 20YY
/// 2. 아니면 19YY 로 읽어 초등학생 나이면 19YY
/// 3. 둘 다 아닌데 20YY 가 **아직 오지 않은 날**이면 19YY — 태어나지도 않은
///    생년월일을 만들어 내지 않기 위해서다 (`850315` → 2085 가 아니라 1985)
/// 4. 그 밖에는 20YY 로 두고, 호출한 쪽이 "확인 필요"를 붙인다
fn guess_century(yy: i32, month: u32, day: u32, today: NaiveDate) -> i32 {
    let date_of = |year: i32| NaiveDate::from_ymd_opt(year, month, day);
    let in_range = |year: i32| {
        date_of(year)
            .map(|d| (AGE_MIN..=AGE_MAX).contains(&age_on(d, today)))
            .unwrap_or(false)
    };

    if in_range(2000 + yy) {
        2000 + yy
    } else if in_range(1900 + yy) {
        1900 + yy
    } else if date_of(2000 + yy).map(|d| d > today).unwrap_or(false) {
        1900 + yy
    } else {
        2000 + yy
    }
}

/// `today` 기준 만 나이.
pub fn age_on(birth: NaiveDate, today: NaiveDate) -> i32 {
    let mut age = today.year() - birth.year();
    if (today.month(), today.day()) < (birth.month(), birth.day()) {
        age -= 1;
    }
    age
}

/// 화면 표시 형식 `17.03.15.`
///
/// 화면은 TypeScript 쪽에서 같은 규칙으로 만든다. 이 함수는 엑셀 내보내기(Phase 8)가 쓴다.
#[allow(dead_code)]
pub fn format_display(date: NaiveDate) -> String {
    format!(
        "{:02}.{:02}.{:02}.",
        date.year() % 100,
        date.month(),
        date.day()
    )
}

/// DB 저장 형식 `2017-03-15`
pub fn format_iso(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

#[cfg(test)]
#[path = "birth_tests.rs"]
mod birth_tests;
