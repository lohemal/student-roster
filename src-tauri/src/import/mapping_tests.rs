//! 열 짝짓기 추측 검사.

use super::*;

fn headers(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn col(m: &Mapping, f: Field) -> Option<usize> {
    m.get(&f).copied()
}

#[test]
fn 흔한_열_이름을_알아본다() {
    let h = headers(&[
        "학년", "반", "번호", "이름", "성별", "생년월일", "주소",
        "부 성명", "모 성명", "부 연락처", "모 연락처", "보호자 연락처", "비고",
    ]);
    let m = guess(&h);
    assert_eq!(col(&m, Field::Grade), Some(0));
    assert_eq!(col(&m, Field::ClassName), Some(1));
    assert_eq!(col(&m, Field::ClassNo), Some(2));
    assert_eq!(col(&m, Field::Name), Some(3));
    assert_eq!(col(&m, Field::Gender), Some(4));
    assert_eq!(col(&m, Field::Birth), Some(5));
    assert_eq!(col(&m, Field::Address), Some(6));
    assert_eq!(col(&m, Field::FatherName), Some(7));
    assert_eq!(col(&m, Field::MotherName), Some(8));
    assert_eq!(col(&m, Field::FatherPhone), Some(9));
    assert_eq!(col(&m, Field::MotherPhone), Some(10));
    assert_eq!(col(&m, Field::PrimaryPhone), Some(11));
    assert_eq!(col(&m, Field::Note), Some(12));
}

#[test]
fn 학교마다_다른_이름도_알아본다() {
    let m = guess(&headers(&["학급", "성명", "출석번호", "생일", "거주지"]));
    assert_eq!(col(&m, Field::ClassName), Some(0));
    assert_eq!(col(&m, Field::Name), Some(1));
    assert_eq!(col(&m, Field::ClassNo), Some(2));
    assert_eq!(col(&m, Field::Birth), Some(3));
    assert_eq!(col(&m, Field::Address), Some(4));
}

#[test]
fn 아버지_어머니_표현도_알아본다() {
    let m = guess(&headers(&["아버지", "어머니", "아버지 연락처", "어머니 전화"]));
    assert_eq!(col(&m, Field::FatherName), Some(0));
    assert_eq!(col(&m, Field::MotherName), Some(1));
    assert_eq!(col(&m, Field::FatherPhone), Some(2));
    assert_eq!(col(&m, Field::MotherPhone), Some(3));
}

#[test]
fn 공백과_괄호가_섞여_있어도_알아본다() {
    let m = guess(&headers(&[" 성  명 ", "생년월일(8자리)", "부.연락처"]));
    assert_eq!(col(&m, Field::Name), Some(0));
    assert_eq!(col(&m, Field::Birth), Some(1));
    assert_eq!(col(&m, Field::FatherPhone), Some(2));
}

#[test]
fn 한_열이_두_항목에_겹쳐_배정되지_않는다() {
    // '부 연락처' 는 부 성명(별칭 '부')에도 걸릴 수 있다
    let m = guess(&headers(&["부 연락처", "모 연락처"]));
    assert_eq!(col(&m, Field::FatherPhone), Some(0));
    assert_eq!(col(&m, Field::MotherPhone), Some(1));

    let mut used: Vec<usize> = m.values().copied().collect();
    used.sort_unstable();
    let before = used.len();
    used.dedup();
    assert_eq!(used.len(), before, "한 열이 두 번 쓰이면 안 된다");
}

#[test]
fn 모르는_열은_짝짓지_않는다() {
    let m = guess(&headers(&["이름", "알수없는열", "메모장"]));
    assert_eq!(col(&m, Field::Name), Some(0));
    assert!(!m.values().any(|&c| c == 1), "모르는 열은 비워 둔다");
}

#[test]
fn 빈_헤더는_짝짓지_않는다() {
    let m = guess(&headers(&["", "  ", "이름"]));
    assert_eq!(col(&m, Field::Name), Some(2));
    assert_eq!(m.len(), 1);
}

#[test]
fn 딱_맞는_이름을_덧말_붙은_것보다_먼저_고른다() {
    // '이름' 과 '학생이름' 이 함께 있으면 딱 맞는 '이름' 을 고른다
    let m = guess(&headers(&["학생이름", "이름"]));
    assert_eq!(col(&m, Field::Name), Some(1));
}

// ---------- 헤더 줄 찾기 ----------

#[test]
fn 제목_줄을_건너뛰고_헤더를_찾는다() {
    let rows = vec![
        (1, headers(&["2026학년도 학생명단", "", "", ""])),
        (2, headers(&["", "", "", ""])),
        (3, headers(&["학년", "반", "번호", "이름"])),
        (4, headers(&["1", "가람", "1", "홍길동"])),
    ];
    // 빈 줄은 read_sheet 에서 이미 빠지므로 여기서는 두 줄째가 헤더다
    let rows: Vec<_> = rows
        .into_iter()
        .filter(|(_, c)| c.iter().any(|x| !x.is_empty()))
        .collect();
    assert_eq!(guess_header_row(&rows, 10), 1);
}

#[test]
fn 첫_줄이_헤더면_그대로_고른다() {
    let rows = vec![
        (1, headers(&["학년", "반", "번호", "이름"])),
        (2, headers(&["1", "가람", "1", "홍길동"])),
    ];
    assert_eq!(guess_header_row(&rows, 10), 0);
}

#[test]
fn 아는_열이_하나도_없으면_첫_줄로_본다() {
    let rows = vec![
        (1, headers(&["A", "B"])),
        (2, headers(&["1", "2"])),
    ];
    assert_eq!(guess_header_row(&rows, 10), 0);
}
