//! 생년월일 정규화 검사.
//!
//! 기준일은 2026-09-15 로 고정한다. '오늘'에 따라 결과가 바뀌면 안 되는 부분과
//! 바뀌어야 하는 부분(세기 판단)을 분명히 나누기 위해서다.

use super::*;

fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 9, 15).unwrap()
}

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

// ---------- 여러 가지 모양을 같은 날짜로 읽는다 ----------

#[test]
fn 요구사항에_적힌_여섯_가지_모양을_모두_읽는다() {
    let expected = d(2017, 3, 15);
    for raw in [
        "170315",
        "17.03.15",
        "17.03.15.",
        "17-03-15",
        "20170315",
        "2017.03.15",
    ] {
        let r = parse(raw, today());
        assert_eq!(r.date, Some(expected), "{raw} 를 읽지 못했다");
        assert_eq!(r.problem, None, "{raw} 에 불필요한 확인 필요가 붙었다");
    }
}

#[test]
fn 구분자가_무엇이든_숫자만_보면_된다() {
    let expected = d(2017, 3, 5);
    for raw in [
        "2017/03/05",
        "2017년 03월 05일",
        " 2017 . 03 . 05 ",
        "170305",
        "17/3/5",
    ] {
        assert_eq!(parse(raw, today()).date, Some(expected), "{raw}");
    }
}

// ---------- 6자리 세기 판단 ----------

#[test]
fn 여섯자리는_초등학생_나이에_맞는_세기로_읽는다() {
    // 2026년 기준 초등학생은 2010~2022년생 언저리다.
    assert_eq!(parse("150820", today()).date, Some(d(2015, 8, 20)));
    assert_eq!(parse("200101", today()).date, Some(d(2020, 1, 1)));
}

#[test]
fn 이십세기로_읽어야_말이_되는_값은_십구백년대로_읽는다() {
    // 2085 년생은 아직 태어나지 않았다. 1985 년생이어야 말이 된다.
    // (초등학생 범위는 아니므로 확인 필요는 붙지만, 세기 판단은 1900년대여야 한다.)
    let r = parse("850315", today());
    assert_eq!(r.date, Some(d(1985, 3, 15)));
    assert!(matches!(r.problem, Some(BirthProblem::OutOfRange { .. })));
}

#[test]
fn 아직_오지_않은_생년월일은_만들지_않는다() {
    // 2085 년은 미래다. 태어나지 않은 날짜를 만드느니 1985 로 읽는다.
    for (raw, expected) in [
        ("850315", d(1985, 3, 15)),
        ("991231", d(1999, 12, 31)),
        ("300101", d(1930, 1, 1)),
    ] {
        let r = parse(raw, today());
        assert_eq!(r.date, Some(expected), "{raw}");
        assert!(r.date.unwrap() <= today(), "{raw} 가 미래 날짜가 되었다");
    }
}

#[test]
fn 한자리_월일도_잘못_읽지_않는다() {
    // '2017/3/5' 의 숫자만 모으면 201735 라 여섯 자리가 된다.
    // 구분자를 보지 않으면 20년 17월 35일로 읽어 버린다.
    assert_eq!(parse("2017/3/5", today()).date, Some(d(2017, 3, 5)));
    assert_eq!(parse("17.3.5", today()).date, Some(d(2017, 3, 5)));
    assert_eq!(parse("2017-3-15", today()).date, Some(d(2017, 3, 15)));
}

#[test]
fn 어느_세기로도_초등학생이_아니면_이천년대로_두고_확인을_요청한다() {
    // 2005 년생(만 21세)도 1905 년생도 초등학생이 아니다.
    let r = parse("050315", today());
    assert_eq!(r.date, Some(d(2005, 3, 15)), "임의로 1905 를 만들지 않는다");
    assert!(matches!(r.problem, Some(BirthProblem::OutOfRange { age: 21 })));
}

// ---------- 달력에 없는 날짜 ----------

#[test]
fn 달력에_없는_날짜는_고치지_않고_확인_필요로_남긴다() {
    for raw in ["20170230", "170230", "20171301", "20170000", "171332"] {
        let r = parse(raw, today());
        assert_eq!(r.date, None, "{raw} 를 임의의 날짜로 바꾸면 안 된다");
        assert_eq!(
            r.problem,
            Some(BirthProblem::NotACalendarDate),
            "{raw} 는 달력에 없는 날짜로 표시되어야 한다"
        );
    }
}

#[test]
fn 윤년은_제대로_가린다() {
    // 2016 은 윤년, 2017 은 아니다
    assert_eq!(parse("20160229", today()).date, Some(d(2016, 2, 29)));
    assert_eq!(parse("20170229", today()).date, None);
}

// ---------- 읽을 수 없는 값 ----------

#[test]
fn 자릿수가_맞지_않으면_읽지_못한_것으로_본다() {
    for raw in ["1703", "1703150", "2017", "17031", "abc", "-"] {
        let r = parse(raw, today());
        assert_eq!(r.date, None, "{raw}");
        assert_eq!(r.problem, Some(BirthProblem::Unreadable), "{raw}");
    }
}

#[test]
fn 빈_값은_문제로_보지_않는다() {
    for raw in ["", "   ", "\t"] {
        let r = parse(raw, today());
        assert_eq!(r.date, None);
        assert_eq!(r.problem, None, "비어 있는 것 자체는 생년월일 오류가 아니다");
        assert!(is_blank(raw));
    }
}

// ---------- 나이 범위 ----------

#[test]
fn 초등학생_나이_경계를_확인한다() {
    // 만 4세 ~ 만 15세 까지 정상
    let four = d(2022, 9, 15); // 2026-09-15 기준 만 4세
    let fifteen = d(2011, 9, 15); // 만 15세
    assert_eq!(parse(&format_iso(four), today()).problem, None);
    assert_eq!(parse(&format_iso(fifteen), today()).problem, None);

    // 만 3세 / 만 16세는 확인 필요
    let three = d(2023, 9, 16);
    let sixteen = d(2010, 9, 14);
    assert!(parse(&format_iso(three), today()).problem.is_some());
    assert!(parse(&format_iso(sixteen), today()).problem.is_some());
}

#[test]
fn 나이_범위를_벗어나도_날짜값은_버리지_않는다() {
    let r = parse("20000101", today());
    assert_eq!(r.date, Some(d(2000, 1, 1)), "읽은 날짜는 남겨 둔다");
    assert!(r.problem.is_some());
}

#[test]
fn 생일_전후로_만_나이가_달라진다() {
    let birth = d(2017, 12, 25);
    assert_eq!(age_on(birth, d(2026, 12, 24)), 8, "생일 전날");
    assert_eq!(age_on(birth, d(2026, 12, 25)), 9, "생일 당일");
}

// ---------- 표시 ----------

#[test]
fn 화면에는_점_세_개_형식으로_보여준다() {
    assert_eq!(format_display(d(2017, 3, 15)), "17.03.15.");
    assert_eq!(format_display(d(2009, 12, 31)), "09.12.31.");
    assert_eq!(format_display(d(2000, 1, 1)), "00.01.01.");
}

#[test]
fn 저장은_국제_표준_형식으로_한다() {
    assert_eq!(format_iso(d(2017, 3, 15)), "2017-03-15");
}

#[test]
fn 읽은_값을_다시_읽어도_같다() {
    for raw in ["170315", "20111231", "2020.02.29"] {
        let first = parse(raw, today()).date;
        if let Some(date) = first {
            let again = parse(&format_iso(date), today()).date;
            assert_eq!(again, first, "{raw} 를 두 번 읽으면 달라진다");
        }
    }
}

#[test]
fn 확인_필요_사유에는_사람이_읽을_문장이_있다() {
    for p in [
        BirthProblem::Unreadable,
        BirthProblem::NotACalendarDate,
        BirthProblem::OutOfRange { age: 21 },
    ] {
        let m = p.message();
        assert!(!m.is_empty());
        assert!(m.ends_with('.') || m.contains("예:"), "{m}");
    }
}
