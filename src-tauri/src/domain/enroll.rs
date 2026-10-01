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

/// **기준일 현재 재학생** — SQL 조건을 그대로 끼워 넣는 조각이다.
///
/// `alias` 는 `"e."` 처럼 점까지 포함해 넘기고, 별칭이 없으면 빈 문자열을 준다.
///
/// 규칙은 두 줄이다.
///   * 전입일이 **아직 오지 않았으면** 재학생이 아니다 (전입 예정).
///     전입일 **당일부터** 든다.
///   * 전출일이 **아직 오지 않았으면** 그때까지는 재학생이다 (전출 예정).
///     전출일 **당일부터** 빠진다.
///
/// 날짜가 바뀌는 자정에 DB 를 고치지 않는다. 저장된 상태·이동일과 기준일로 **셀 때마다
/// 계산**하므로, 앱이 꺼져 있던 동안 날이 바뀌어도 다음 실행에서 바로 맞는다.
///
/// 기준일은 바인딩 번호를 밀지 않도록 글자로 끼워 넣는다 — 값은 우리가 만든
/// `YYYY-MM-DD` 열 글자뿐이라 바깥에서 들어온 글자가 섞일 길이 없다.
///
/// 이 조각을 쓰는 곳: 학생명단·검색·통계·형제·확인 필요·내보내기·주소 재적용·
/// 번호 재정렬·반별 인원·학년도 인원·학년도 전환. 조건을 바꾸려면 여기만 고친다.
pub fn active_sql(alias: &str, asof: NaiveDate) -> String {
    let d = asof.format("%Y-%m-%d");
    format!(
        "(({a}status <> 'TRANSFER_OUT' \
             OR ({a}transfer_out_date IS NOT NULL AND {a}transfer_out_date > '{d}')) \
          AND ({a}status <> 'TRANSFER_IN' \
             OR {a}transfer_in_date IS NULL OR {a}transfer_in_date <= '{d}'))",
        a = alias
    )
}

/// **그 학년도에 적을 두고 있는 학생** — SQL 조건 조각.
///
/// `active_sql` 과 한 가지가 다르다: **아직 오지 않은 전입생도 든다.** 이미 나간
/// 학생만 뺀다.
///
/// 형제 후보를 찾을 때 쓴다. 후보 판정은 '오늘 교실에 있는가' 가 아니라 '이 학년도
/// 명단에 이름이 있는가' 의 문제이고, 전입 예정 학생을 빼 두면 그 학생이 오는 날
/// 아무도 다시 훑어 주지 않아 후보가 영영 생기지 않는다. 반대로 **'본교 형제 수'**
/// 처럼 지금을 말해야 하는 숫자는 `active_on` 으로 따로 가린다.
pub fn enrolled_sql(alias: &str, asof: NaiveDate) -> String {
    let d = asof.format("%Y-%m-%d");
    format!(
        "({a}status <> 'TRANSFER_OUT' \
            OR ({a}transfer_out_date IS NOT NULL AND {a}transfer_out_date > '{d}'))",
        a = alias
    )
}

/// `active_sql` 과 **같은 판정을 Rust 쪽에서** 한다.
///
/// 둘이 어긋나면 화면마다 숫자가 달라지므로 검사에서 둘을 맞대어 본다.
pub fn active_on(
    status: &str,
    transfer_in_date: Option<&str>,
    transfer_out_date: Option<&str>,
    asof: NaiveDate,
) -> bool {
    let d = asof.format("%Y-%m-%d").to_string();
    let out_ok = status != "TRANSFER_OUT"
        || transfer_out_date.map(|o| o > d.as_str()).unwrap_or(false);
    let in_ok = status != "TRANSFER_IN"
        || transfer_in_date.map(|i| i <= d.as_str()).unwrap_or(true);
    out_ok && in_ok
}

/// 아직 오지 않은 이동.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pending {
    /// 전입 예정 — 그날이 되어야 명단에 든다
    In(NaiveDate),
    /// 전출 예정 — 그날부터 명단에서 빠진다
    Out(NaiveDate),
}

impl Pending {
    pub fn label(self) -> &'static str {
        match self {
            Pending::In(_) => "전입 예정",
            Pending::Out(_) => "전출 예정",
        }
    }

    /// 화면이 분기할 때 쓰는 코드
    pub fn code(self) -> &'static str {
        match self {
            Pending::In(_) => "IN",
            Pending::Out(_) => "OUT",
        }
    }

    pub fn date(self) -> NaiveDate {
        match self {
            Pending::In(d) | Pending::Out(d) => d,
        }
    }
}

/// 기준일에 아직 오지 않은 이동이 있는가.
pub fn pending_on(
    status: &str,
    transfer_in_date: Option<&str>,
    transfer_out_date: Option<&str>,
    asof: NaiveDate,
) -> Option<Pending> {
    let later = |raw: Option<&str>| raw.and_then(parse_date).filter(|d| *d > asof);
    match status {
        "TRANSFER_IN" => later(transfer_in_date).map(Pending::In),
        "TRANSFER_OUT" => later(transfer_out_date).map(Pending::Out),
        _ => None,
    }
}

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
    /// DB 에 적히는 글자. 저장 경로는 `ACTIVE_STATUS_SQL` 과 각 repo 의 SQL 이 쓰므로
    /// 지금은 검사에서 `parse` 와 짝이 맞는지 보는 데만 쓴다.
    #[cfg(test)]
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

    /// 상태 글자만 보았을 때 다니는 쪽인가.
    ///
    /// **명단·통계가 세는 기준은 이것이 아니라 `active_on`/`active_sql` 이다** —
    /// 전입 예정·전출 예정은 날짜까지 보아야 가려진다. 그래서 쓰이는 곳이 없고,
    /// 상태 글자와 뜻이 어긋나지 않는지 보는 검사만 쓴다.
    #[cfg(test)]
    pub fn is_active(self) -> bool {
        !matches!(self, Status::TransferOut)
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
///
/// **아직 오지 않은 날도 받는다** — 학교는 전입·전출 예정일을 미리 알고 며칠 전에
/// 적어 두는 일이 많다. 대신 학년도 범위는 그대로 막으므로 다음 학년도까지 넘어가는
/// 예정은 들어오지 못한다. 그날이 되기 전까지의 취급은 `pending_on` 이 정한다.
pub fn check_date(
    raw: &str,
    school_year: i32,
    last: Option<(&str, NaiveDate)>,
) -> Result<NaiveDate, DateProblem> {
    let d = parse_date(raw).ok_or(DateProblem::Unreadable)?;

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
