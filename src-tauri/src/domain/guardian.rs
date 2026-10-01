//! 주보호자 연락처 — 어디서 온 번호인가, 한꺼번에 바꾸면 무슨 일이 생기는가.
//!
//! **출처를 따로 저장하지 않는다.** `students` 에는 `primary_phone` 값만 있고, 그 값이
//! 모 연락처·부 연락처와 같은지를 **볼 때마다 견주어** `[모]`·`[부]` 를 붙인다.
//!
//! 왜 저장하지 않는가
//!   * 저장하면 "주보호자 = 모" 라는 관계가 남아, 모 연락처를 고칠 때 **문자가 나가는
//!     번호가 말없이 따라 바뀐다.** 학교가 고친 것은 모 연락처 한 칸인데 결과가 둘이다.
//!   * 견주어 보여 주면 모 연락처가 바뀐 순간 `[모]` 표시가 사라진다 — 주보호자 번호가
//!     더 이상 모의 번호가 아니라는 **사실 그대로**다. 사람이 보고 다시 정하면 된다.
//!   * 값만 보면 되므로 `001_init.sql` 을 열 까닭이 없다(v0.1.0 동결).
//!
//! 견줄 때는 **숫자만 남긴 값**(`phone::digits`)을 쓴다. `010-1234-5678` 과
//! `01012345678` 은 같은 번호다.

/// 주보호자 연락처가 어디서 온 번호인가.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// 모 연락처와 같다
    Mother,
    /// 부 연락처와 같다
    Father,
    /// 부·모 연락처가 같은 번호라 어느 쪽이라고 말할 수 없다
    Both,
    /// 부·모 어느 쪽과도 다른 번호 (조부모·본인 등)
    Other,
    /// 주보호자 연락처가 없다
    None,
}

impl Source {
    /// 화면이 분기할 때 쓰는 코드
    pub fn code(self) -> &'static str {
        match self {
            Source::Mother => "MOTHER",
            Source::Father => "FATHER",
            Source::Both => "BOTH",
            Source::Other => "OTHER",
            Source::None => "NONE",
        }
    }

    /// 번호 뒤에 붙일 짧은 말. 색만으로 뜻을 전하지 않도록 **언제나 글자**로 준다.
    pub fn label(self) -> Option<&'static str> {
        match self {
            Source::Mother => Some("모"),
            Source::Father => Some("부"),
            Source::Both => Some("모·부"),
            Source::Other => Some("기타"),
            Source::None => None,
        }
    }
}

fn same(a: Option<&str>, b: Option<&str>) -> bool {
    match (a, b) {
        (Some(x), Some(y)) => !x.is_empty() && x == y,
        _ => false,
    }
}

/// 주보호자 연락처의 출처를 견주어 가린다. 모두 **숫자만 남긴 값**으로 넘긴다.
pub fn source(
    primary_digits: Option<&str>,
    mother_digits: Option<&str>,
    father_digits: Option<&str>,
) -> Source {
    let primary = primary_digits.filter(|d| !d.is_empty());
    let Some(p) = primary else {
        return Source::None;
    };
    let is_mother = same(Some(p), mother_digits);
    let is_father = same(Some(p), father_digits);
    match (is_mother, is_father) {
        // 부·모가 같은 번호를 쓰고 있다 — 어느 쪽이라고 **임의로 고르지 않는다**
        (true, true) => Source::Both,
        (true, false) => Source::Mother,
        (false, true) => Source::Father,
        (false, false) => Source::Other,
    }
}

// ---------------------------------------------------------------
// 한꺼번에 설정하기
// ---------------------------------------------------------------

/// 어느 연락처를 주보호자로 삼을 것인가.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FillFrom {
    Mother,
    Father,
}

impl FillFrom {
    pub fn parse(code: &str) -> Option<Self> {
        match code {
            "MOTHER" => Some(FillFrom::Mother),
            "FATHER" => Some(FillFrom::Father),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            FillFrom::Mother => "모 연락처",
            FillFrom::Father => "부 연락처",
        }
    }

    /// 읽어 올 열 이름 — 표시용과 숫자용 짝
    pub fn columns(self) -> (&'static str, &'static str) {
        match self {
            FillFrom::Mother => ("mother_phone", "mother_phone_digits"),
            FillFrom::Father => ("father_phone", "father_phone_digits"),
        }
    }
}

/// 학생 한 명에게 일괄 설정이 무슨 일을 하는가.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// 가져올 연락처 자체가 없다 — **건드리지 않는다**
    Missing,
    /// 이미 같은 번호다 — 할 일이 없다
    Already,
    /// 주보호자 칸이 비어 있어 새로 채운다
    FillEmpty,
    /// 다른 번호가 들어 있어 **덮어쓴다**
    Overwrite,
}

/// 숫자만 남긴 값으로 견주어 무슨 일이 일어날지 가린다.
///
/// **가져올 연락처가 없다고 주보호자 칸을 비우지 않는다.** 비우는 쪽이 더 큰 일이고,
/// 사용자가 시킨 것은 '이 번호로 채워라' 이지 '지워라' 가 아니다.
pub fn outcome(source_digits: Option<&str>, primary_digits: Option<&str>) -> Outcome {
    let src = source_digits.filter(|d| !d.is_empty());
    let Some(s) = src else {
        return Outcome::Missing;
    };
    match primary_digits.filter(|d| !d.is_empty()) {
        None => Outcome::FillEmpty,
        Some(p) if p == s => Outcome::Already,
        Some(_) => Outcome::Overwrite,
    }
}

#[cfg(test)]
#[path = "guardian_tests.rs"]
mod guardian_tests;
