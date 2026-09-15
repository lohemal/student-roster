//! 학적 상태·이동 날짜 규칙 검사.

use super::*;

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

fn today() -> NaiveDate {
    d(2026, 9, 15)
}

// ---------------------------------------------------------------
// 상태
// ---------------------------------------------------------------

#[test]
fn 전입생은_재학생에_들고_전출생은_빠진다() {
    assert!(Status::Enrolled.is_active());
    assert!(
        Status::TransferIn.is_active(),
        "전입생은 지금 학교에 다닌다"
    );
    assert!(!Status::TransferOut.is_active(), "전출생은 명단에서 빠진다");
}

#[test]
fn 상태_이름과_값이_서로_맞는다() {
    for s in [Status::Enrolled, Status::TransferIn, Status::TransferOut] {
        assert_eq!(Status::parse(s.code()), Some(s));
        assert!(!s.label().is_empty());
    }
    assert_eq!(Status::parse("무엇"), None);
}

#[test]
fn 재학생_조건이_두_상태를_모두_담는다() {
    assert!(ACTIVE_STATUS_SQL.starts_with("status IN"), "조건 전체를 담는다");
    assert!(ACTIVE_STATUS_SQL.contains("ENROLLED"));
    assert!(ACTIVE_STATUS_SQL.contains("TRANSFER_IN"));
    assert!(
        !ACTIVE_STATUS_SQL.contains("TRANSFER_OUT"),
        "전출이 들어가면 온 화면의 숫자가 어긋난다"
    );
}

// ---------------------------------------------------------------
// 학년도 범위
// ---------------------------------------------------------------

#[test]
fn 학년도는_삼월에_시작해_다음_해_이월에_끝난다() {
    let (from, to) = year_range(2026);
    assert_eq!(from, d(2026, 3, 1));
    assert_eq!(to, d(2027, 2, 28));

    // 윤년도 따로 따지지 않고 맞는다
    let (_, leap) = year_range(2027);
    assert_eq!(leap, d(2028, 2, 29));
}

// ---------------------------------------------------------------
// 이동 날짜
// ---------------------------------------------------------------

#[test]
fn 학년도_안의_지난_날짜는_받는다() {
    assert_eq!(
        check_date("2026-09-10", 2026, today(), None),
        Ok(d(2026, 9, 10))
    );
    assert_eq!(
        check_date("2026-03-02", 2026, today(), None),
        Ok(d(2026, 3, 2))
    );
    assert_eq!(
        check_date("2026-09-15", 2026, today(), None),
        Ok(today()),
        "오늘도 받는다"
    );
}

#[test]
fn 날짜로_읽지_못하면_거절한다() {
    for bad in ["", "어제", "2026/09/10", "20260910", "2026-13-40"] {
        assert_eq!(
            check_date(bad, 2026, today(), None),
            Err(DateProblem::Unreadable),
            "{bad}"
        );
    }
}

#[test]
fn 아직_오지_않은_날은_거절한다() {
    assert_eq!(
        check_date("2026-09-16", 2026, today(), None),
        Err(DateProblem::Future)
    );
}

#[test]
fn 학년도_밖의_날짜는_거절한다() {
    // 2026학년도는 2026-03-01 부터다
    assert_eq!(
        check_date("2026-02-28", 2026, today(), None),
        Err(DateProblem::OutOfYear)
    );
    // 지난 학년도 날짜도 마찬가지
    assert_eq!(
        check_date("2025-09-10", 2026, today(), None),
        Err(DateProblem::OutOfYear)
    );
}

#[test]
fn 앞선_이동보다_이른_날짜는_거절한다() {
    let last = ("전출", d(2026, 5, 14));
    let err = check_date("2026-04-01", 2026, today(), Some(last)).unwrap_err();
    assert_eq!(
        err,
        DateProblem::BeforeLast {
            last_label: "전출".into(),
            last_date: d(2026, 5, 14)
        }
    );

    // 같은 날은 받는다 — 하루에 나갔다 들어오는 일이 없지는 않다
    assert!(check_date("2026-05-14", 2026, today(), Some(last)).is_ok());
    assert!(check_date("2026-09-10", 2026, today(), Some(last)).is_ok());
}

#[test]
fn 오류_문구가_무엇을_고쳐야_할지_알려_준다() {
    let msg = DateProblem::OutOfYear.message("전입일", 2026);
    assert!(msg.contains("전입일"));
    assert!(msg.contains("2026"), "어느 학년도인지 알려 준다");
    assert!(msg.contains("2027.02.28."), "언제까지인지 알려 준다");

    let msg = DateProblem::BeforeLast {
        last_label: "전출".into(),
        last_date: d(2026, 5, 14),
    }
    .message("전입일", 2026);
    assert!(msg.contains("2026.05.14."), "무엇보다 이른지 알려 준다");
}

#[test]
fn 날짜_표시는_점으로_끝난다() {
    assert_eq!(format_date(d(2026, 9, 5)), "2026.09.05.");
}
