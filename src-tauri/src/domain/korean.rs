//! 한국어 조사 고르기.
//!
//! 안내 문구에 `주소이(가) 비어 있습니다` 처럼 나오면 읽기 나쁘다.
//! 앞말의 **받침 여부**로 조사를 골라 `주소가 비어 있습니다` 로 만든다.
//!
//! 완성형 한글은 유니코드에서 `0xAC00 + (초성×588) + (중성×28) + 종성` 으로 놓여 있어
//! 28로 나눈 나머지가 0이 아니면 받침이 있다.

/// 받침이 있으면 `with`, 없으면 `without`.
pub fn particle<'a>(word: &str, with: &'a str, without: &'a str) -> &'a str {
    match word.trim().chars().last() {
        Some(c) => {
            let code = c as u32;
            if (0xAC00..=0xD7A3).contains(&code) {
                if (code - 0xAC00) % 28 == 0 {
                    without
                } else {
                    with
                }
            } else {
                // 한글이 아니면(숫자·영문) 읽는 법이 갈리므로 안전한 쪽으로 둔다
                with
            }
        }
        None => with,
    }
}

/// 주격 조사 — `성별이` / `주소가`
pub fn subject(word: &str) -> &'static str {
    particle(word, "이", "가")
}

/// 보조사 — `성별은` / `주소는`
#[allow(dead_code)] // 뒤 Phase 의 안내 문구에서 쓴다
pub fn topic(word: &str) -> &'static str {
    particle(word, "은", "는")
}

/// 목적격 조사 — `성별을` / `주소를`
#[allow(dead_code)]
pub fn object(word: &str) -> &'static str {
    particle(word, "을", "를")
}

/// `성별, 주소가 비어 있습니다.` 처럼 항목을 잇고 조사를 붙인다.
pub fn list_with_subject(items: &[&str], tail: &str) -> String {
    let joined = items.join(", ");
    let last = items.last().copied().unwrap_or("");
    format!("{joined}{} {tail}", subject(last))
}

#[cfg(test)]
#[path = "korean_tests.rs"]
mod korean_tests;
