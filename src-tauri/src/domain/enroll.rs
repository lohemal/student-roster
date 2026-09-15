//! 학적 상태와 이동 날짜 규칙.
//!
//! **현재 재학생이 무엇인지 한 곳에서 정한다.** 명단·통계·형제·번호·주소가 모두 같은
//! 뜻으로 세어야 숫자가 어긋나지 않는다.
//!
//! 지키는 것
//!   * 전입생은 재학생에 **든다**. 전출생은 **빠진다**.
//!   * 전출했다고 학생을 지우지 않는다. 상태만 바뀐다.
//!   * 이동 날짜는 이력의 본체다. 이력을 깨뜨리는 날짜는 받지 않는다.

use chrono::{Datelike, NaiveDate};

/// **현재 재학생** — 재학 + 전입. SQL 조건을 그대로 끼워 넣는 조각이다.
///
/// 별칭이 있는 곳에서는 `e.{ACTIVE}` 처럼 앞에 붙여 쓴다.
///
/// 이 값을 쓰는 곳: 학생명단·주소 재적용·형제 탐색·번호 재정렬·반별 인원·학년도 인원.
/// 조건을 바꾸려면 여기 한 줄만 고친다.
pub const ACTIVE_STATUS_SQL: &str = "status IN ('ENROLLED','TRANSFER_IN')";

/// 학적 상태.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// 그냥 다니는 학생
    Enrolled,
    /// 이 학년도에 들어온 학생. **재학생에 든다.**
    TransferIn,
    /// 나간 학생. 재학생 명단에서 빠지지만 자료는 남는다.
    TransferOut,
}

impl Status {
    pub fn code(self) -> &'static str {
        match self {
            Status::Enrolled => "ENROLLED",
            Status::TransferIn => "TRANSFER_IN",
            Status::TransferOut => "TRANSFER_OUT",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Status::Enrolled => "재학",
            Status::TransferIn => "전입",
            Status::TransferOut => "전출",
        }
    }

    pub fn parse(code: &str) -> Option<Self> {
        match code {
            "ENROLLED" => Some(Status::Enrolled),
            "TRANSFER_IN" => Some(Status::TransferIn),
            "TRANSFER_OUT" => Some(Status::TransferOut),
            _ => None,
        }
    }

    /// 지금 학교에 다니는가 — 명단·통계에 세는 기준.
    pub fn is_active(self) -> bool {
        !matches!(self, Status::TransferOut)
    }
}

/// 이동 사건 종류. `enrollment_events.kind` 와 같은 값이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Enroll,
    TransferIn,
    TransferOut,
    Promote,
    Graduate,
    Cancel,
}

impl Event {
    pub fn code(self) -> &'static str {
        match self {
            Event::Enroll => "ENROLL",
            Event::TransferIn => "TRANSFER_IN",
            Event::TransferOut => "TRANSFER_OUT",
            Event::Promote => "PROMOTE",
            Event::Graduate => "GRADUATE",
            Event::Cancel => "CANCEL",
        }
    }
}

// ---------------------------------------------------------------
// 학년도 범위
// ---------------------------------------------------------------

/// 그 학년도가 걸쳐 있는 날짜 범위. **3월 1일 ~ 다음 해 2월 말일.**
pub fn year_range(school_year: i32) -> (NaiveDate, NaiveDate) {
    let from = NaiveDate::from_ymd_opt(school_year, 3, 1).expect("3월 1일은 언제나 있다");
    // 다음 해 3월 1일의 하루 전 = 2월 말일 (윤년을 따로 따지지 않아도 된다)
    let next = NaiveDate::from_ymd_opt(school_year + 1, 3, 1).expect("3월 1일은 언제나 있다");
    (from, next.pred_opt().expect("3월 1일 앞에는 날이 있다"))
}

/// 사람이 읽는 학년도 범위. 오류 문구에 쓴다.
pub fn year_range_label(school_year: i32) -> String {
    let (from, to) = year_range(school_year);
    format!(
        "{}.{:02}.{:02}.~{}.{:02}.{:02}.",
        from.year(),
        from.month(),
        from.day(),
        to.year(),
        to.month(),
        to.day()
    )
}

// ---------------------------------------------------------------
// 이동 날짜 검증
// ---------------------------------------------------------------

/// 이동 날짜를 받을 수 없는 까닭.
///
/// 빠진 값이 있어도 저장을 막지 않는 것이 이 프로그램의 원칙이지만, **이동 날짜는
/// 이 작업의 본체**다. 날짜가 틀리면 남는 이력 자체가 거짓이 되므로 여기서는 막는다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DateProblem {
    /// 날짜로 읽을 수 없다
    Unreadable,
    /// 아직 오지 않은 날
    Future,
    /// 그 학년도에 들어 있지 않은 날
    OutOfYear,
    /// 앞선 이동보다 이르다 (전출 뒤에 그보다 이른 전입 등)
    BeforeLast {
        last_label: String,
        last_date: NaiveDate,
    },
}

impl DateProblem {
    pub fn message(&self, what: &str, school_year: i32) -> String {
        match self {
            DateProblem::Unreadable => {
                format!("{what}을 날짜로 읽지 못했습니다. 2026-09-15 처럼 적어 주세요.")
            }
            DateProblem::Future => {
                format!("{what}이 아직 오지 않은 날입니다. 오늘까지의 날짜로 적어 주세요.")
            }
            DateProblem::OutOfYear => format!(
                "{what}이 {school_year}학년도({})에 들어 있지 않습니다.",
                year_range_label(school_year)
            ),
            DateProblem::BeforeLast {
                last_label,
                last_date,
            } => format!(
                "{what}이 앞선 {last_label}({})보다 이릅니다. 이동 순서가 뒤바뀝니다.",
                format_date(*last_date)
            ),
        }
    }
}

/// `2026.09.15.`
pub fn format_date(d: NaiveDate) -> String {
    format!("{}.{:02}.{:02}.", d.year(), d.month(), d.day())
}

pub fn parse_date(raw: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(raw.trim(), "%Y-%m-%d").ok()
}

/// 이동 날짜를 따진다.
///
/// `last` 는 같은 학년도의 **가장 마지막 이동**(종류 이름, 날짜)이다. 없으면 None.
pub fn check_date(
    raw: &str,
    school_year: i32,
    today: NaiveDate,
    last: Option<(&str, NaiveDate)>,
) -> Result<NaiveDate, DateProblem> {
    let d = parse_date(raw).ok_or(DateProblem::Unreadable)?;

    if d > today {
        return Err(DateProblem::Future);
    }
    let (from, to) = year_range(school_year);
    if d < from || d > to {
        return Err(DateProblem::OutOfYear);
    }
    if let Some((label, last_date)) = last {
        if d < last_date {
            return Err(DateProblem::BeforeLast {
                last_label: label.to_string(),
                last_date,
            });
        }
    }
    Ok(d)
}

#[cfg(test)]
#[path = "enroll_tests.rs"]
mod enroll_tests;
