//! 주보호자 연락처 규칙 검사. 번호는 모두 가상이다.

use super::*;

const MOM: &str = "01010002222";
const DAD: &str = "01010003333";
const GRANNY: &str = "01010004444";

// ---------------------------------------------------------------
// 출처 가리기
// ---------------------------------------------------------------

#[test]
fn 주보호자가_모_연락처와_같으면_모로_본다() {
    assert_eq!(source(Some(MOM), Some(MOM), Some(DAD)), Source::Mother);
    assert_eq!(Source::Mother.label(), Some("모"));
    assert_eq!(Source::Mother.code(), "MOTHER");
}

#[test]
fn 주보호자가_부_연락처와_같으면_부로_본다() {
    assert_eq!(source(Some(DAD), Some(MOM), Some(DAD)), Source::Father);
    assert_eq!(Source::Father.label(), Some("부"));
}

#[test]
fn 부모_어느_쪽과도_다르면_기타다() {
    assert_eq!(source(Some(GRANNY), Some(MOM), Some(DAD)), Source::Other);
    assert_eq!(Source::Other.label(), Some("기타"));
    // 견줄 값이 아예 없어도 억지로 부·모라고 하지 않는다
    assert_eq!(source(Some(GRANNY), None, None), Source::Other);
}

#[test]
fn 부모가_같은_번호면_한쪽을_고르지_않는다() {
    assert_eq!(source(Some(MOM), Some(MOM), Some(MOM)), Source::Both);
    assert_eq!(Source::Both.label(), Some("모·부"));
}

#[test]
fn 주보호자_연락처가_없으면_표시할_것도_없다() {
    assert_eq!(source(None, Some(MOM), Some(DAD)), Source::None);
    assert_eq!(source(Some(""), Some(MOM), Some(DAD)), Source::None);
    assert_eq!(Source::None.label(), None);
}

#[test]
fn 빈값끼리는_같다고_보지_않는다() {
    // 모 연락처도 주보호자도 비어 있다 — '모와 같다' 가 아니다
    assert_eq!(source(Some(""), Some(""), Some("")), Source::None);
}

// ---------------------------------------------------------------
// 일괄 설정이 할 일
// ---------------------------------------------------------------

#[test]
fn 가져올_연락처가_없으면_건드리지_않는다() {
    assert_eq!(outcome(None, Some(GRANNY)), Outcome::Missing);
    assert_eq!(outcome(Some(""), Some(GRANNY)), Outcome::Missing);
    assert_eq!(outcome(None, None), Outcome::Missing);
}

#[test]
fn 비어_있으면_채우고_같으면_그대로_둔다() {
    assert_eq!(outcome(Some(MOM), None), Outcome::FillEmpty);
    assert_eq!(outcome(Some(MOM), Some("")), Outcome::FillEmpty);
    assert_eq!(outcome(Some(MOM), Some(MOM)), Outcome::Already);
}

#[test]
fn 다른_번호가_들어_있으면_덮어쓰는_일이라고_가린다() {
    assert_eq!(outcome(Some(MOM), Some(DAD)), Outcome::Overwrite);
    assert_eq!(outcome(Some(MOM), Some(GRANNY)), Outcome::Overwrite);
}

#[test]
fn 어느_연락처를_쓸지는_코드로_주고받는다() {
    assert_eq!(FillFrom::parse("MOTHER"), Some(FillFrom::Mother));
    assert_eq!(FillFrom::parse("FATHER"), Some(FillFrom::Father));
    assert_eq!(FillFrom::parse("GRANNY"), None);
    assert_eq!(FillFrom::Mother.label(), "모 연락처");
    assert_eq!(FillFrom::Mother.columns(), ("mother_phone", "mother_phone_digits"));
    assert_eq!(FillFrom::Father.columns(), ("father_phone", "father_phone_digits"));
}
