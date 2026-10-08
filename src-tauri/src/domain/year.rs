//! 학년도를 지워도 되는가 — 판정 규칙만 둔다.
//!
//! 학년도 삭제는 되돌리기 어려운 작업이라 **무엇을 보고 막는지가 한곳에 적혀 있어야**
//! 한다. DB 를 읽는 일은 `repo::year` 가 하고, 여기서는 읽어 온 사실만 보고 가린다.
//!
//! 지키는 것
//!   * **시스템 날짜의 연도로 가리지 않는다.** 학교가 정한 '현재 학년도' 가 기준이다.
//!     1~2월에는 달력 연도와 학년도가 다르고, 미리 만들어 둔 다음 학년도를 지우는 것이
//!     이 기능의 목적이기 때문이다.
//!   * 현재 학년도는 지울 수 없다. 프로그램이 쓸 학년도가 없어지면 안 된다.
//!   * 이미 지나간 학년도도 지울 수 없다. 잘못 만든 학년도를 되돌리는 기능이지
//!     운영 기록을 지우는 기능이 아니다.
//!   * 졸업 기록이 걸린 학년도는 지울 수 없다. 졸업은 그 해에 실제로 일어난 일이다.

/// 삭제를 막은 까닭.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// 그런 학년도가 없다
    NotFound,
    /// 현재 학년도가 아직 정해지지 않았다
    NoCurrent,
    /// 지금 쓰고 있는 학년도다
    Current,
    /// 현재 학년도이거나 그보다 앞이다 — 괄호 안은 현재 학년도
    NotFuture(i32),
    /// 이 학년도를 졸업 학년도로 하는 졸업 기록이 있다
    Graduations(i64),
    /// 이 학년도보다 뒤에 학적이 남아 있는 학년도가 있다
    LaterYear(i32),
}

impl Refusal {
    pub fn code(&self) -> &'static str {
        match self {
            Refusal::NotFound => "NOT_FOUND",
            Refusal::NoCurrent => "NO_CURRENT",
            Refusal::Current => "CURRENT",
            Refusal::NotFuture(_) => "NOT_FUTURE",
            Refusal::Graduations(_) => "GRADUATIONS",
            Refusal::LaterYear(_) => "LATER_YEAR",
        }
    }

    /// 왜 막혔는지와 **무엇을 하면 되는지**를 함께 말한다.
    pub fn message(&self, year: i32) -> String {
        match self {
            Refusal::NotFound => format!("{year}학년도가 없습니다."),
            Refusal::NoCurrent => {
                "현재 학년도가 정해져 있지 않습니다. 먼저 현재 학년도를 지정해 주세요."
                    .to_string()
            }
            Refusal::Current => format!(
                "{year}학년도는 지금 사용 중인 학년도라 삭제할 수 없습니다. \
                 다른 학년도를 현재 학년도로 지정한 뒤 다시 시도해 주세요."
            ),
            Refusal::NotFuture(cur) => format!(
                "{year}학년도는 이미 지나간 학년도입니다. \
                 현재 학년도({cur}학년도)보다 뒤의 학년도만 삭제할 수 있습니다."
            ),
            Refusal::Graduations(n) => format!(
                "{year}학년도 졸업 기록이 {n}건 있습니다. 실제로 운영한 학년도는 삭제할 수 없습니다."
            ),
            Refusal::LaterYear(later) => format!(
                "{later}학년도에 학적이 남아 있습니다. 뒤의 학년도부터 삭제해 주세요."
            ),
        }
    }
}

/// 판정에 쓰는 사실. `repo::year` 가 DB 에서 읽어 채운다.
#[derive(Debug, Clone, Default)]
pub struct Facts {
    /// `school_years` 에 있는가
    pub exists: bool,
    /// 학교가 정한 현재 학년도
    pub current: Option<i32>,
    /// 이 학년도를 졸업 학년도로 하는 졸업 기록 수
    pub graduations: i64,
    /// 이 학년도보다 뒤이면서 학적이 남아 있는 학년도 (가장 가까운 것)
    pub later_with_enrollments: Option<i32>,
}

/// 지울 수 있으면 `Ok`, 아니면 까닭.
pub fn check(year: i32, f: &Facts) -> Result<(), Refusal> {
    if !f.exists {
        return Err(Refusal::NotFound);
    }
    let current = f.current.ok_or(Refusal::NoCurrent)?;
    if year == current {
        return Err(Refusal::Current);
    }
    if year < current {
        return Err(Refusal::NotFuture(current));
    }
    if f.graduations > 0 {
        return Err(Refusal::Graduations(f.graduations));
    }
    if let Some(later) = f.later_with_enrollments {
        return Err(Refusal::LaterYear(later));
    }
    Ok(())
}

#[cfg(test)]
#[path = "year_tests.rs"]
mod year_tests;
