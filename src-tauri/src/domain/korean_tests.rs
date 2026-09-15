//! 조사 고르기 검사. 화면에 그대로 나가는 문구라 어색하면 눈에 띈다.

use super::*;

#[test]
fn 받침이_있으면_이_없으면_가() {
    // 받침 있음
    for w in ["성별", "반", "생년월일", "이름", "학년", "번호판"] {
        assert_eq!(subject(w), "이", "{w}");
    }
    // 받침 없음
    for w in ["주소", "번호", "연락처", "보호자 연락처", "비고"] {
        assert_eq!(subject(w), "가", "{w}");
    }
}

#[test]
fn 실제로_쓰는_항목_이름이_자연스럽다() {
    assert_eq!(format!("성별{}", subject("성별")), "성별이");
    assert_eq!(format!("주소{}", subject("주소")), "주소가");
    assert_eq!(format!("번호{}", subject("번호")), "번호가");
    assert_eq!(
        format!("보호자 연락처{}", subject("보호자 연락처")),
        "보호자 연락처가"
    );
}

#[test]
fn 다른_조사도_고른다() {
    assert_eq!(topic("성별"), "은");
    assert_eq!(topic("주소"), "는");
    assert_eq!(object("성별"), "을");
    assert_eq!(object("주소"), "를");
}

#[test]
fn 한글이_아니면_안전한_쪽으로_둔다() {
    assert_eq!(subject("A"), "이");
    assert_eq!(subject("2"), "이");
    assert_eq!(subject(""), "이");
}

#[test]
fn 여러_항목을_이을_때는_마지막_말을_본다() {
    assert_eq!(
        list_with_subject(&["성별", "주소"], "비어 있습니다."),
        "성별, 주소가 비어 있습니다."
    );
    assert_eq!(
        list_with_subject(&["주소", "성별"], "비어 있습니다."),
        "주소, 성별이 비어 있습니다."
    );
    assert_eq!(
        list_with_subject(&["번호"], "정해지지 않았습니다."),
        "번호가 정해지지 않았습니다."
    );
}

#[test]
fn 뒤에_붙는_말은_그대로_둔다() {
    let m = list_with_subject(&["반", "번호"], "정해지지 않았습니다.");
    assert!(m.ends_with("정해지지 않았습니다."));
    assert!(m.starts_with("반, 번호"));
}
