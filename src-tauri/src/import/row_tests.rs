//! 엑셀 한 줄 읽기 검사.

use super::*;
use crate::import::mapping::{Field, Mapping};

fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 9, 15).unwrap()
}

/// 학년·반·번호·이름·성별·생년월일·주소·부연락처 순서의 표
fn standard_mapping() -> Mapping {
    let mut m = Mapping::new();
    m.insert(Field::Grade, 0);
    m.insert(Field::ClassName, 1);
    m.insert(Field::ClassNo, 2);
    m.insert(Field::Name, 3);
    m.insert(Field::Gender, 4);
    m.insert(Field::Birth, 5);
    m.insert(Field::Address, 6);
    m.insert(Field::FatherPhone, 7);
    m
}

fn cells(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn read(v: &[&str]) -> ParsedRow {
    parse(2, &cells(v), &standard_mapping(), None, today())
}

#[test]
fn 보통의_한_줄을_읽는다() {
    let r = read(&[
        "3", "가람", "7", "홍길동", "남", "170315",
        "○○시 ○○로 123", "010-1234-5678",
    ]);
    assert_eq!(r.name, "홍길동");
    assert_eq!(r.grade, Some(3));
    assert_eq!(r.class_name.as_deref(), Some("가람"));
    assert_eq!(r.class_no, Some(7));
    assert_eq!(r.gender.as_deref(), Some("M"));
    assert_eq!(r.birth_date, NaiveDate::from_ymd_opt(2017, 3, 15));
    assert_eq!(r.address.as_deref(), Some("○○시 ○○로 123"));
    assert_eq!(r.blocker, None);
}

#[test]
fn 학년에_글자가_붙어_있어도_읽는다() {
    assert_eq!(read(&["3학년", "가람", "7", "홍길동"]).grade, Some(3));
    assert_eq!(read(&[" 3 ", "가람", "7", "홍길동"]).grade, Some(3));
}

#[test]
fn 번호에_번이_붙어_있어도_읽는다() {
    assert_eq!(read(&["3", "가람", "7번", "홍길동"]).class_no, Some(7));
}

#[test]
fn 성별을_여러_표기로_읽는다() {
    for m in ["남", "남자", "M", "m", "1"] {
        assert_eq!(read(&["1", "가람", "1", "이름", m]).gender.as_deref(), Some("M"), "{m}");
    }
    for f in ["여", "여자", "F", "f", "2"] {
        assert_eq!(read(&["1", "가람", "1", "이름", f]).gender.as_deref(), Some("F"), "{f}");
    }
    assert_eq!(read(&["1", "가람", "1", "이름", ""]).gender, None);
    assert_eq!(read(&["1", "가람", "1", "이름", "미상"]).gender, None);
}

#[test]
fn 반은_숫자든_글자든_그대로_둔다() {
    assert_eq!(read(&["1", "1", "1", "이름"]).class_name.as_deref(), Some("1"));
    assert_eq!(read(&["1", "가람", "1", "이름"]).class_name.as_deref(), Some("가람"));
}

#[test]
fn 연락처의_앞자리_영을_되살려_읽는다() {
    let r = read(&["1", "가람", "1", "이름", "남", "", "", "1012345678"]);
    assert_eq!(r.father_phone.as_deref(), Some("01012345678"));
}

#[test]
fn 빈칸은_없는_값으로_둔다() {
    let r = read(&["1", "가람", "", "이름", "", "", "  ", ""]);
    assert_eq!(r.class_no, None);
    assert_eq!(r.gender, None);
    assert_eq!(r.birth_raw, None);
    assert_eq!(r.address, None);
    assert_eq!(r.father_phone, None);
    assert_eq!(r.blocker, None, "빈칸이 있다고 막지 않는다");
}

#[test]
fn 짝짓지_않은_항목은_비워_둔다() {
    let mut m = Mapping::new();
    m.insert(Field::Name, 0);
    let r = parse(2, &cells(&["홍길동", "쓰지않는열"]), &m, Some(4), today());
    assert_eq!(r.name, "홍길동");
    assert_eq!(r.grade, Some(4), "학년 열이 없으면 정해 둔 학년을 쓴다");
    assert_eq!(r.class_name, None);
    assert_eq!(r.note, None);
}

// ---------- 가져올 수 없는 줄 ----------

#[test]
fn 이름이_없으면_가져오지_않는다() {
    let r = read(&["3", "가람", "7", ""]);
    assert!(r.blocker.as_deref().unwrap().contains("이름"));
}

#[test]
fn 학년을_알_수_없으면_가져오지_않는다() {
    let r = read(&["", "가람", "7", "홍길동"]);
    assert!(r.blocker.as_deref().unwrap().contains("학년"));
}

#[test]
fn 학년이_범위를_벗어나면_가져오지_않는다() {
    assert!(read(&["7", "가람", "7", "홍길동"]).blocker.is_some());
    assert!(read(&["0", "가람", "7", "홍길동"]).blocker.is_some());
}

#[test]
fn 번호가_범위를_벗어나면_가져오지_않는다() {
    assert!(read(&["3", "가람", "999", "홍길동"]).blocker.is_some());
}

#[test]
fn 생년월일이_이상해도_막지_않는다() {
    let r = read(&["3", "가람", "7", "홍길동", "남", "20170230"]);
    assert_eq!(r.blocker, None, "생년월일 때문에 줄을 버리지 않는다");
    assert_eq!(r.birth_date, None);
    assert_eq!(r.birth_raw.as_deref(), Some("20170230"), "원본은 남는다");
    assert!(r.birth_problem.is_some());
}

// ---------- 미리 알리는 확인거리 ----------

#[test]
fn 빠진_항목을_미리_알려_준다() {
    let r = read(&["3", "가람", "", "홍길동"]);
    let found = warnings(&r);
    let joined = found
        .iter()
        .map(|w| w.message.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(joined.contains("번호"), "{joined}");
    assert!(joined.contains("성별"), "{joined}");
    assert!(joined.contains("보호자 연락처"), "{joined}");
}

#[test]
fn 다_채워져_있으면_알릴_것이_없다() {
    let r = read(&[
        "3", "가람", "7", "홍길동", "남", "170315",
        "○○시 ○○로 123", "010-1234-5678",
    ]);
    assert!(warnings(&r).is_empty());
}

#[test]
fn 생년월일_문제는_따로_알려_준다() {
    let r = read(&["3", "가람", "7", "홍길동", "남", "20170230", "주소", "010-1234-5678"]);
    let msgs = warnings(&r);
    assert!(msgs.iter().any(|w| w.message.contains("달력에 없는")), "{msgs:?}");
}

// ---------- 저장할 값으로 옮기기 ----------

#[test]
fn 저장할_값으로_그대로_옮긴다() {
    let r = read(&[
        "3", "가람", "7", "홍길동", "남", "170315",
        "○○시 ○○로 123", "010-1234-5678",
    ]);
    let input = to_input(&r, 2026);
    assert_eq!(input.name, "홍길동");
    assert_eq!(input.school_year, 2026);
    assert_eq!(input.grade, 3);
    assert_eq!(input.class_name.as_deref(), Some("가람"));
    assert_eq!(input.class_no, Some(7));
    assert_eq!(input.birth_raw.as_deref(), Some("170315"));
    assert!(input.validate().is_ok());
}
