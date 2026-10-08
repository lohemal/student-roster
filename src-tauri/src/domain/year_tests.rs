use super::*;

fn facts(current: i32) -> Facts {
    Facts {
        exists: true,
        current: Some(current),
        ..Default::default()
    }
}

#[test]
fn 현재보다_뒤_학년도는_지울_수_있다() {
    assert_eq!(check(2027, &facts(2026)), Ok(()));
}

#[test]
fn 현재_학년도는_못_지운다() {
    let err = check(2026, &facts(2026)).unwrap_err();
    assert_eq!(err.code(), "CURRENT");
    assert!(
        err.message(2026).contains("다른 학년도를 현재 학년도로"),
        "무엇을 하면 되는지 알려 줘야 한다: {}",
        err.message(2026)
    );
}

#[test]
fn 지나간_학년도는_못_지운다() {
    let err = check(2025, &facts(2026)).unwrap_err();
    assert_eq!(err.code(), "NOT_FUTURE");
    assert!(err.message(2025).contains("2026학년도"), "기준을 알려 준다");
}

#[test]
fn 현재_학년도가_없으면_판정하지_않는다() {
    let f = Facts {
        exists: true,
        current: None,
        ..Default::default()
    };
    // 달력 연도로 가리지 않는다 — 기준이 없으면 막는다
    assert_eq!(check(2030, &f).unwrap_err().code(), "NO_CURRENT");
}

#[test]
fn 없는_학년도() {
    let f = Facts {
        exists: false,
        current: Some(2026),
        ..Default::default()
    };
    assert_eq!(check(2027, &f).unwrap_err().code(), "NOT_FOUND");
}

#[test]
fn 졸업_기록이_걸린_학년도는_못_지운다() {
    let f = Facts {
        graduations: 48,
        ..facts(2026)
    };
    let err = check(2027, &f).unwrap_err();
    assert_eq!(err.code(), "GRADUATIONS");
    assert!(err.message(2027).contains("48건"));
}

#[test]
fn 뒤에_학적이_남은_학년도가_있으면_그것부터() {
    let f = Facts {
        later_with_enrollments: Some(2028),
        ..facts(2026)
    };
    let err = check(2027, &f).unwrap_err();
    assert_eq!(err.code(), "LATER_YEAR");
    assert!(err.message(2027).contains("2028학년도"));
}

#[test]
fn 달력_연도가_아니라_현재_학년도로_가린다() {
    // 2026학년도를 쓰는 동안 2026년 12월이든 2027년 1월이든 판정은 같다
    assert_eq!(check(2027, &facts(2026)), Ok(()));
    assert_eq!(check(2026, &facts(2027)).unwrap_err().code(), "NOT_FUTURE");
}
