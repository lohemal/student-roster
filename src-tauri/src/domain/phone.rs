//! 연락처 정규화.
//!
//! 저장은 두 벌로 한다.
//!   * `display` — 사람이 읽는 값 `010-1234-5678`
//!   * `digits`  — 숫자만 `01012345678` (검색·형제 비교에 쓴다)
//!
//! 검색은 `digits LIKE '%1234%'` 한 번이면 되므로 가운데 네 자리든 끝 네 자리든
//! 똑같이 찾힌다. 모양이 이상한 번호라고 저장을 막지는 않는다 — 원문을 그대로
//! `display` 에 두고 숫자만 뽑아 둔다.

/// 숫자만 남긴다. 검색어에도 같은 함수를 쓴다.
pub fn digits(raw: &str) -> String {
    raw.chars().filter(|c| c.is_ascii_digit()).collect()
}

/// 저장할 두 값을 만든다. 빈 값이면 둘 다 None.
pub fn normalize(raw: &str) -> (Option<String>, Option<String>) {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return (None, None);
    }
    let d = digits(trimmed);
    if d.is_empty() {
        // 숫자가 하나도 없다 — 원문만 남긴다
        return (Some(trimmed.to_string()), None);
    }
    (Some(format_display(&d, trimmed)), Some(d))
}

/// 보기 좋은 형태로 만든다. 아는 모양이 아니면 원문을 그대로 쓴다.
fn format_display(d: &str, original: &str) -> String {
    match d.len() {
        // 휴대전화 010-1234-5678 / 011-123-4567
        11 if d.starts_with("01") => format!("{}-{}-{}", &d[0..3], &d[3..7], &d[7..11]),
        10 if d.starts_with("01") => format!("{}-{}-{}", &d[0..3], &d[3..6], &d[6..10]),
        // 서울 02-123-4567 / 02-1234-5678
        10 if d.starts_with("02") => format!("{}-{}-{}", &d[0..2], &d[2..6], &d[6..10]),
        9 if d.starts_with("02") => format!("{}-{}-{}", &d[0..2], &d[2..5], &d[5..9]),
        // 그 밖의 지역번호 044-123-4567
        10 => format!("{}-{}-{}", &d[0..3], &d[3..6], &d[6..10]),
        11 => format!("{}-{}-{}", &d[0..3], &d[3..7], &d[7..11]),
        // 아는 모양이 아니다 — 사람이 적은 그대로 둔다
        _ => original.to_string(),
    }
}

/// 검색어를 숫자로 바꾼다. 숫자가 너무 짧으면 None (온 학교가 걸리는 검색을 막는다).
pub fn search_digits(term: &str, min_len: usize) -> Option<String> {
    let d = digits(term);
    if d.len() >= min_len {
        Some(d)
    } else {
        None
    }
}

/// 형제 판정에 쓸 수 있는 값인지. 너무 짧은 숫자는 비교 대상이 아니다.
#[allow(dead_code)] // Phase 4 형제 판정에서 쓴다
pub fn is_comparable(digits: &Option<String>) -> bool {
    digits.as_ref().map(|d| d.len() >= 9).unwrap_or(false)
}

#[cfg(test)]
#[path = "phone_tests.rs"]
mod phone_tests;
