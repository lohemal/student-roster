//! 엑셀 칸 읽기 검사.

use super::*;
use calamine::{ExcelDateTime, ExcelDateTimeType};

#[test]
fn 글자_칸은_앞뒤_공백을_턴다() {
    assert_eq!(cell_to_string(&Data::String("  홍길동 ".into())), "홍길동");
    assert_eq!(cell_to_string(&Data::String("".into())), "");
}

#[test]
fn 숫자_칸에_소수점이_붙지_않는다() {
    // 엑셀에서 170315 를 넣으면 실수로 읽히는 일이 잦다
    assert_eq!(cell_to_string(&Data::Float(170315.0)), "170315");
    assert_eq!(cell_to_string(&Data::Int(7)), "7");
    assert_eq!(cell_to_string(&Data::Float(3.0)), "3");
}

#[test]
fn 빈_칸과_오류_칸은_빈_값이다() {
    assert_eq!(cell_to_string(&Data::Empty), "");
    assert_eq!(
        cell_to_string(&Data::Error(calamine::CellErrorType::NA)),
        "",
        "#N/A 는 값이 없는 것으로 본다"
    );
}

#[test]
fn 날짜_칸은_연월일로_읽는다() {
    // 엑셀 일련값 42800 = 2017-03-15
    let dt = ExcelDateTime::new(42809.0, ExcelDateTimeType::DateTime, false);
    let s = cell_to_string(&Data::DateTime(dt));
    assert_eq!(s.len(), 10, "YYYY-MM-DD 모양이어야 한다: {s}");
    assert!(s.starts_with("2017"), "{s}");

    // 이 값을 그대로 생년월일로 읽을 수 있어야 한다
    let today = chrono::NaiveDate::from_ymd_opt(2026, 9, 15).unwrap();
    assert!(crate::domain::birth::parse(&s, today).date.is_some());
}

#[test]
fn iso_날짜_칸은_시각을_뗀다() {
    assert_eq!(
        cell_to_string(&Data::DateTimeIso("2017-03-15T00:00:00".into())),
        "2017-03-15"
    );
}

// ---------- 연락처 앞자리 0 ----------

#[test]
fn 숫자로_들어온_연락처의_앞자리_영을_되살린다() {
    // 엑셀에서 010-1234-5678 을 숫자로 넣으면 1012345678 이 된다
    assert_eq!(restore_phone_leading_zero("1012345678"), "01012345678");
    assert_eq!(restore_phone_leading_zero("212345678"), "0212345678");
}

#[test]
fn 이미_영으로_시작하면_그대로_둔다() {
    assert_eq!(restore_phone_leading_zero("01012345678"), "01012345678");
    assert_eq!(restore_phone_leading_zero("0441234567"), "0441234567");
}

#[test]
fn 글자가_섞여_있으면_건드리지_않는다() {
    assert_eq!(restore_phone_leading_zero("010-1234-5678"), "010-1234-5678");
    assert_eq!(restore_phone_leading_zero("없음"), "없음");
    assert_eq!(restore_phone_leading_zero(""), "");
}

#[test]
fn 자릿수가_맞지_않으면_건드리지_않는다() {
    assert_eq!(restore_phone_leading_zero("1234"), "1234");
    assert_eq!(restore_phone_leading_zero("123456789012"), "123456789012");
    assert_eq!(restore_phone_leading_zero("912345678"), "912345678", "9로 시작");
}

#[test]
fn 되살린_번호가_제대로_읽힌다() {
    let restored = restore_phone_leading_zero("1012345678");
    let (display, digits) = crate::domain::phone::normalize(&restored);
    assert_eq!(display.as_deref(), Some("010-1234-5678"));
    assert_eq!(digits.as_deref(), Some("01012345678"));
}

// ---------- 없는 파일 ----------

#[test]
fn 없는_파일은_알아들을_수_있는_말로_알린다() {
    let err = inspect("C:/이런/파일은/없다.xlsx").unwrap_err();
    assert_eq!(err.code, "NOT_FOUND");
    assert!(err.user_message.contains("찾을 수 없"));
}

#[test]
fn 엑셀이_아닌_파일은_열지_않는다() {
    let dir = std::env::temp_dir().join("roster-excel-test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("가짜.xlsx");
    std::fs::write(&path, b"this is not an excel file").unwrap();

    let err = inspect(path.to_str().unwrap()).unwrap_err();
    assert_eq!(err.code, "INVALID_INPUT");
    assert!(err.user_message.contains("열지 못했습니다"));
    let _ = std::fs::remove_file(&path);
}
