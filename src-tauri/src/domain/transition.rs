//! 학년도 전환 규칙 — 누가 올라가고, 누가 졸업하고, 새 번호를 어떻게 매기는가.
//!
//! 이 모듈은 **지난 학년도를 고치지 않는다.** 다음 학년도에 만들 학적이 어떤
//! 모습이어야 하는지만 계산한다. 2026학년도 3-가람-7 은 그대로 두고 2027학년도
//! 4-나리-12 를 새로 만드는 것이 이 작업의 전부다.
//!
//! 지키는 것
//!   * **프로그램이 반을 정하지 않는다.** 새 반은 학교가 정해 파일로 넣는다.
//!     빠진 반을 예전 반으로 채우거나 임의로 배정하지 않는다.
//!   * **일부만 입력된 번호를 프로그램이 채우지 않는다.** 전부 비어 있을 때만
//!     이름 차례로 매긴다. 반쯤 입력된 번호를 채우면 사람이 의도한 자리가 깨진다.
//!   * 6학년은 다음 학적을 만들지 않는다. 대신 졸업이다. **7학년은 없다.**

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// 초등학교 학년 범위.
pub const MIN_GRADE: i32 = 1;
pub const MAX_GRADE: i32 = 6;

/// 한 학생이 다음 학년도에 맞을 일.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// 다음 학년도 학적을 만든다
    Promote(i32),
    /// 다음 학적 없이 졸업
    Graduate,
}

/// 기본 규칙에 따른 다음 학년. 6학년은 없다.
pub fn next_grade(grade: i32) -> Option<i32> {
    (MIN_GRADE..MAX_GRADE).contains(&grade).then_some(grade + 1)
}

/// 기본 처리. 1~5학년은 진급, 6학년은 졸업.
pub fn outcome(grade: i32) -> Outcome {
    match next_grade(grade) {
        Some(g) => Outcome::Promote(g),
        None => Outcome::Graduate,
    }
}

/// 학년으로 쓸 수 있는 값인가. **프로그램이 7학년을 만들지 않는다.**
pub fn is_valid_grade(grade: i32) -> bool {
    (MIN_GRADE..=MAX_GRADE).contains(&grade)
}

// ---------------------------------------------------------------
// 번호 매기기
// ---------------------------------------------------------------

/// 한 반의 새 번호가 어떤 상태인가.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberState {
    /// 아무도 번호가 없다 — 이름 차례로 매겨 준다
    AllEmpty,
    /// 모두 번호가 있다 — 그대로 쓴다 (중복은 따로 본다)
    AllGiven,
    /// 일부만 있다 — **프로그램이 채우지 않는다**
    Partial,
}

pub fn number_state(numbers: &[Option<i32>]) -> NumberState {
    let given = numbers.iter().filter(|n| n.is_some()).count();
    if given == 0 {
        NumberState::AllEmpty
    } else if given == numbers.len() {
        NumberState::AllGiven
    } else {
        NumberState::Partial
    }
}

/// 이름 차례로 1~N 을 매긴다.
///
/// 한글 이름은 글자 차례가 곧 가나다 차례라 명단 정렬(`ORDER BY s.name`)과 같은
/// 규칙이다. 같은 이름이 둘이면 들어온 차례를 지킨다(안정 정렬).
pub fn numbers_by_name(names: &[(i64, String)]) -> Vec<(i64, i32)> {
    let mut sorted: Vec<&(i64, String)> = names.iter().collect();
    sorted.sort_by(|a, b| a.1.cmp(&b.1));
    sorted
        .into_iter()
        .enumerate()
        .map(|(i, (id, _))| (*id, i as i32 + 1))
        .collect()
}

/// 한 반 안에서 두 번 쓰인 번호.
pub fn duplicate_numbers(numbers: &[Option<i32>]) -> Vec<i32> {
    let mut seen: BTreeMap<i32, usize> = BTreeMap::new();
    for n in numbers.iter().flatten() {
        *seen.entry(*n).or_insert(0) += 1;
    }
    seen.into_iter()
        .filter(|(_, n)| *n > 1)
        .map(|(no, _)| no)
        .collect()
}

// ---------------------------------------------------------------
// 진급 배정 양식
// ---------------------------------------------------------------

/// 양식의 열. 차례가 곧 만들어지는 엑셀의 열 차례다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Col {
    SchoolYear,
    StudentId,
    OldGrade,
    OldClass,
    OldNo,
    Name,
    Birth,
    NewGrade,
    NewClass,
    NewNo,
}

impl Col {
    pub const ALL: [Col; 10] = [
        Col::SchoolYear,
        Col::StudentId,
        Col::OldGrade,
        Col::OldClass,
        Col::OldNo,
        Col::Name,
        Col::Birth,
        Col::NewGrade,
        Col::NewClass,
        Col::NewNo,
    ];

    /// 엑셀 머리글. 양식을 만들 때도 읽을 때도 이 글자를 쓴다.
    pub fn header(self) -> &'static str {
        match self {
            Col::SchoolYear => "학년도",
            Col::StudentId => "학생번호",
            Col::OldGrade => "기존 학년",
            Col::OldClass => "기존 반",
            Col::OldNo => "기존 번호",
            Col::Name => "이름",
            Col::Birth => "생년월일",
            Col::NewGrade => "새 학년",
            Col::NewClass => "새 반",
            Col::NewNo => "새 번호",
        }
    }

    pub fn width(self) -> f64 {
        match self {
            Col::Name => 10.0,
            Col::Birth => 12.0,
            Col::OldClass | Col::NewClass => 10.0,
            Col::SchoolYear | Col::StudentId => 9.0,
            _ => 8.0,
        }
    }

}

// ---------------------------------------------------------------
// 확인할 것
// ---------------------------------------------------------------

/// 전환 전에 사람이 봐야 할 것.
///
/// **막는 것과 알리는 것을 나눈다.** 새 반이 없으면 학적을 만들 수 없으니 막지만,
/// 주소 미분류처럼 전환 뒤에 고쳐도 되는 것은 막지 않는다 — 이 프로그램은
/// "오류가 있어도 입력을 막지 않는다" 를 지켜 왔다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProblemKind {
    /// 배정 파일에 없는 학생 (새 반을 알 수 없다)
    NoAssign,
    /// 새 반이 비어 있다
    NoClass,
    /// 같은 학생이 파일에 두 번
    Duplicated,
    /// 파일의 학생을 DB 에서 찾지 못했다
    UnknownStudent,
    /// 이름이 저장된 값과 다르다 — 다른 학생일 수 있다
    NameMismatch,
    /// 쓸 수 없는 새 학년 (1~6 밖)
    BadGrade,
    /// 한 반에 번호가 일부만 있다
    PartialNumbers,
    /// 한 반에 같은 번호가 둘
    NumberDup,
    /// 파일의 학년도가 원본 학년도와 다르다
    WrongYear,
    /// 파일의 기존 학년·반·번호가 지금 학적과 다르다 (알리기만)
    StaleSeat,
    /// 기본 규칙(+1)과 다른 새 학년 (알리기만)
    UnusualGrade,
    /// 졸업에서 뺐는데 다음 학적도 없다 (알리기만)
    NoNextEnrollment,
    /// 대상 학년도에 이미 학적이 있다
    TargetNotEmpty,
    /// 같은 전환을 이미 한 번 돌렸다
    AlreadyDone,
}

impl ProblemKind {
    /// 이것이 있으면 전환을 시작하지 않는다.
    pub fn blocking(self) -> bool {
        !matches!(
            self,
            ProblemKind::StaleSeat
                | ProblemKind::UnusualGrade
                | ProblemKind::NoNextEnrollment
        )
    }

    pub fn label(self) -> &'static str {
        match self {
            ProblemKind::NoAssign => "배정 자료에 없는 학생",
            ProblemKind::NoClass => "새 반 미정",
            ProblemKind::Duplicated => "같은 학생이 두 번",
            ProblemKind::UnknownStudent => "찾을 수 없는 학생",
            ProblemKind::NameMismatch => "이름 불일치",
            ProblemKind::BadGrade => "쓸 수 없는 새 학년",
            ProblemKind::PartialNumbers => "새 번호가 일부만 입력됨",
            ProblemKind::NumberDup => "새 번호 중복",
            ProblemKind::WrongYear => "학년도 불일치",
            ProblemKind::StaleSeat => "기존 학적이 파일과 다름",
            ProblemKind::UnusualGrade => "기본 진급과 다른 학년",
            ProblemKind::NoNextEnrollment => "다음 학년도 학적 없음",
            ProblemKind::TargetNotEmpty => "대상 학년도에 이미 학적이 있음",
            ProblemKind::AlreadyDone => "이미 실행한 전환",
        }
    }
}

// ---------------------------------------------------------------
// 원본이 그대로인가
// ---------------------------------------------------------------

/// 원본 학년도 학적 하나. 미리보기를 만든 뒤 바뀌었는지 보는 데 쓴다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seat {
    pub student_id: i64,
    pub grade: i32,
    pub class_name: Option<String>,
    pub class_no: Option<i32>,
    pub status: String,
}

/// 원본 학년도의 지금 상태를 나타내는 짧은 글.
///
/// 미리보기를 만든 뒤 학생이 전입·전출하거나 반이 바뀌었는데 그대로 적용하면
/// 사람이 본 것과 다른 결과가 만들어진다. 번호 재정렬(`renumber::state_key`)과
/// 같은 방법을 쓴다.
pub fn state_key(seats: &[Seat]) -> String {
    let mut rows: Vec<(i64, i32, String, i32, String)> = seats
        .iter()
        .map(|s| {
            (
                s.student_id,
                s.grade,
                s.class_name.clone().unwrap_or_default(),
                s.class_no.unwrap_or(0),
                s.status.clone(),
            )
        })
        .collect();
    rows.sort_unstable();

    // FNV-1a 64. 되살릴 목적이 아니라 '달라졌는지'만 본다.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |b: u8| {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    };
    for (id, grade, class_name, no, status) in &rows {
        for b in id.to_le_bytes() {
            eat(b);
        }
        for b in grade.to_le_bytes() {
            eat(b);
        }
        for b in class_name.as_bytes() {
            eat(*b);
        }
        for b in no.to_le_bytes() {
            eat(b);
        }
        for b in status.as_bytes() {
            eat(*b);
        }
        eat(b';');
    }
    format!("{}-{:016x}", rows.len(), h)
}

#[cfg(test)]
#[path = "transition_tests.rs"]
mod transition_tests;
