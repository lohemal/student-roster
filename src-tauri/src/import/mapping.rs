//! 엑셀 열과 프로그램 항목 짝짓기.
//!
//! 학교마다 열 이름이 조금씩 다르다(이름/성명/학생명, 반/학급, …).
//! 별칭표로 **추측만** 하고, 확정은 사람이 한다 — 잘못 짝지으면 자료가 통째로 어긋나므로
//! 자동 판정을 그대로 믿지 않는다.

use serde::{Deserialize, Serialize};

/// 프로그램이 받는 항목. 열 하나에 하나씩 대응한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Field {
    Grade,
    ClassName,
    ClassNo,
    Name,
    Gender,
    Birth,
    Address,
    FatherName,
    MotherName,
    FatherPhone,
    MotherPhone,
    PrimaryPhone,
    Note,
}

impl Field {
    pub const ALL: [Field; 13] = [
        Field::Grade,
        Field::ClassName,
        Field::ClassNo,
        Field::Name,
        Field::Gender,
        Field::Birth,
        Field::Address,
        Field::FatherName,
        Field::MotherName,
        Field::FatherPhone,
        Field::MotherPhone,
        Field::PrimaryPhone,
        Field::Note,
    ];

    /// 화면에 보여 줄 이름.
    pub fn label(self) -> &'static str {
        match self {
            Field::Grade => "학년",
            Field::ClassName => "반",
            Field::ClassNo => "번호",
            Field::Name => "이름",
            Field::Gender => "성별",
            Field::Birth => "생년월일",
            Field::Address => "주소",
            Field::FatherName => "부 성명",
            Field::MotherName => "모 성명",
            Field::FatherPhone => "부 연락처",
            Field::MotherPhone => "모 연락처",
            Field::PrimaryPhone => "주보호자 연락처",
            Field::Note => "비고",
        }
    }

    /// 이 항목이 없으면 가져오기를 할 수 없는가.
    pub fn required(self) -> bool {
        matches!(self, Field::Name)
    }

    /// 열 이름 별칭. 왼쪽이 더 정확한 후보다.
    ///
    /// 비교할 때 공백·괄호·마침표를 지우므로 여기에는 붙여 쓴 형태만 적는다.
    fn aliases(self) -> &'static [&'static str] {
        match self {
            Field::Grade => &["학년"],
            Field::ClassName => &["반", "학급", "학급명", "반명"],
            Field::ClassNo => &["번호", "출석번호", "번"],
            Field::Name => &["이름", "성명", "학생명", "학생이름", "성명한글"],
            Field::Gender => &["성별", "남녀"],
            Field::Birth => &["생년월일", "생일", "생년월일자", "출생일", "생년"],
            Field::Address => &["주소", "거주지", "집주소", "주소지", "현주소"],
            Field::FatherName => &["부성명", "부명", "아버지", "아버지성명", "부"],
            Field::MotherName => &["모성명", "모명", "어머니", "어머니성명", "모"],
            Field::FatherPhone => &[
                "부연락처",
                "부전화",
                "부휴대전화",
                "아버지연락처",
                "아버지전화",
                "부핸드폰",
            ],
            Field::MotherPhone => &[
                "모연락처",
                "모전화",
                "모휴대전화",
                "어머니연락처",
                "어머니전화",
                "모핸드폰",
            ],
            Field::PrimaryPhone => &[
                "주보호자연락처",
                "보호자연락처",
                "보호자전화",
                "대표연락처",
                "비상연락처",
                "보호자",
            ],
            Field::Note => &["비고", "특이사항", "메모"],
        }
    }
}

/// 열 이름을 견주기 좋게 다듬는다 — 공백·괄호·마침표·가운뎃점을 지운다.
fn normalize(header: &str) -> String {
    header
        .chars()
        .filter(|c| !c.is_whitespace() && !"()[]{}.·,-_/\\:".contains(*c))
        .collect()
}

/// 한 항목이 그 열 이름과 얼마나 맞는지. 클수록 잘 맞는다. 0이면 맞지 않는다.
fn score(field: Field, header: &str) -> u32 {
    let h = normalize(header);
    if h.is_empty() {
        return 0;
    }
    for (i, alias) in field.aliases().iter().enumerate() {
        // 뒤에 적힌 별칭일수록 점수를 낮춘다
        let rank = 100 - (i as u32 * 5);
        if h == *alias {
            return 1000 + rank; // 딱 맞음
        }
    }
    for (i, alias) in field.aliases().iter().enumerate() {
        let rank = 100 - (i as u32 * 5);
        // '학생 성명(한글)' 처럼 덧말이 붙은 경우.
        // 짧은 별칭(부·모 한 글자)은 아무 데나 걸리므로 포함 비교에서 뺀다.
        if alias.chars().count() >= 2 && h.contains(alias) {
            return 500 + rank;
        }
    }
    0
}

/// 항목 → 열 번호. 값이 없으면 '사용 안 함'.
pub type Mapping = std::collections::HashMap<Field, usize>;

/// 헤더 줄을 보고 짝을 추측한다.
///
/// 한 열이 두 항목에 겹쳐 배정되지 않도록, 점수가 높은 짝부터 하나씩 확정한다.
pub fn guess(headers: &[String]) -> Mapping {
    let mut pairs: Vec<(u32, Field, usize)> = Vec::new();
    for field in Field::ALL {
        for (col, header) in headers.iter().enumerate() {
            let s = score(field, header);
            if s > 0 {
                pairs.push((s, field, col));
            }
        }
    }
    // 점수 높은 순. 같으면 왼쪽 열을 먼저.
    pairs.sort_by(|a, b| b.0.cmp(&a.0).then(a.2.cmp(&b.2)));

    let mut mapping = Mapping::new();
    let mut used: Vec<usize> = Vec::new();
    for (_, field, col) in pairs {
        if mapping.contains_key(&field) || used.contains(&col) {
            continue;
        }
        mapping.insert(field, col);
        used.push(col);
    }
    mapping
}

/// 헤더가 있을 법한 줄을 고른다.
///
/// 맨 윗줄이 제목("2026학년도 학생명단")인 파일이 흔하다. 앞쪽 몇 줄 가운데
/// **아는 열 이름이 가장 많이 걸리는 줄**을 헤더로 본다.
pub fn guess_header_row(rows: &[(usize, Vec<String>)], look: usize) -> usize {
    let mut best = (0usize, 0usize); // (맞은 항목 수, 줄 번호)
    for (i, (_, cells)) in rows.iter().take(look).enumerate() {
        let hits = guess(cells).len();
        if hits > best.0 {
            best = (hits, i);
        }
    }
    best.1
}

#[cfg(test)]
#[path = "mapping_tests.rs"]
mod mapping_tests;
