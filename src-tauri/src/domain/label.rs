//! 학년·반 표시와 정렬 규칙.
//!
//! 반 이름은 숫자(`1`, `2`)일 수도 있고 글자(`가람`, `나리`)일 수도 있다.
//! 그래서 정렬은 **숫자 반 먼저 숫자 순서로, 그 뒤에 글자 반을 가나다 순으로** 한다.
//! (유니코드에서 완성형 한글은 가나다 순서로 늘어서 있어 그냥 비교하면 된다.)

/// 학년-반 표시. `1-나리` / 반이 없으면 `1-미정`.
pub fn class_label(grade: i32, class_name: Option<&str>) -> String {
    match class_name.map(str::trim).filter(|s| !s.is_empty()) {
        Some(c) => format!("{grade}-{c}"),
        None => format!("{grade}-미정"),
    }
}

/// 형제 등 다른 학생을 가리킬 때 쓰는 표시. `1-나리 홍길동`.
#[allow(dead_code)] // Phase 4 형제 표시에서 쓴다
pub fn student_label(grade: i32, class_name: Option<&str>, name: &str) -> String {
    format!("{} {}", class_label(grade, class_name), name)
}

/// 반 정렬 키. 숫자 반이 먼저, 그 다음 글자 반.
///
/// 반이 없는(미정) 학생은 맨 뒤로 보낸다 — 명단을 볼 때 정상 자료가 위에 와야 한다.
pub fn class_sort_key(class_name: Option<&str>) -> (u8, i64, String) {
    match class_name.map(str::trim).filter(|s| !s.is_empty()) {
        None => (2, 0, String::new()),
        Some(c) => match c.parse::<i64>() {
            Ok(n) => (0, n, String::new()),
            Err(_) => (1, 0, c.to_string()),
        },
    }
}

/// 학생명단 기본 정렬 — 학년 → 반 → 번호 → 이름.
///
/// SQL 로 같은 순서를 만들어 내는 조각. 화면이 아니라 DB 에서 정렬해야
/// 1,000명이 넘어도 빠르고, 쪽 나누기(LIMIT/OFFSET)와 어긋나지 않는다.
///
/// 학교가 정한 반 순서(가람·나리·다솜…)를 쓰고 싶어지면, 순서표를 만들어
/// `LEFT JOIN class_orders` 한 뒤 맨 앞에 `COALESCE(co.sort_order, 9999)` 한 줄을
/// 끼워 넣으면 된다. 나머지 규칙은 그대로 둘 수 있다.
pub const ORDER_BY_ROSTER: &str = "
      e.grade,
      CASE WHEN e.class_name IS NULL OR TRIM(e.class_name) = '' THEN 2
           WHEN TRIM(e.class_name) GLOB '[0-9]*' THEN 0
           ELSE 1 END,
      CASE WHEN TRIM(e.class_name) GLOB '[0-9]*' THEN CAST(e.class_name AS INTEGER) ELSE 0 END,
      e.class_name,
      CASE WHEN e.class_no IS NULL THEN 1 ELSE 0 END,
      e.class_no,
      s.name";

#[cfg(test)]
#[path = "label_tests.rs"]
mod label_tests;
