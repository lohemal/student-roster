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
fn 재학생_조건이_기준일과_이동일을_함께_본다() {
    let sql = active_sql("e.", today());
    assert!(sql.starts_with('(') && sql.ends_with(')'), "AND 사이에 끼워도 안전해야 한다");
    assert!(sql.contains("e.status"), "별칭이 붙는다");
    assert!(sql.contains("e.transfer_in_date"));
    assert!(sql.contains("e.transfer_out_date"));
    assert!(sql.contains("2026-09-15"), "기준일이 들어간다");

    let bare = active_sql("", today());
    assert!(bare.contains("status <> 'TRANSFER_OUT'"));
    assert!(!bare.contains("e."), "별칭이 없으면 붙이지 않는다");
}

#[test]
fn 전입일_당일부터_재학생이다() {
    let on = |asof| active_on("TRANSFER_IN", Some("2026-10-05"), None, asof);
    assert!(!on(d(2026, 10, 4)), "전입 예정 — 아직 아니다");
    assert!(on(d(2026, 10, 5)), "당일부터 든다");
    assert!(on(d(2026, 10, 6)));
}

#[test]
fn 전출일_당일부터_재학생에서_빠진다() {
    let on = |asof| active_on("TRANSFER_OUT", None, Some("2026-10-10"), asof);
    assert!(on(d(2026, 10, 9)), "전출 예정 — 아직 다닌다");
    assert!(!on(d(2026, 10, 10)), "당일부터 빠진다");
    assert!(!on(d(2026, 10, 11)));
}

#[test]
fn 날짜가_없으면_예전처럼_상태로만_가린다() {
    // 전입일이 없는 전입생(예전 자료)은 그대로 재학생
    assert!(active_on("TRANSFER_IN", None, None, today()));
    // 전출일이 없는 전출생은 그대로 빠진다
    assert!(!active_on("TRANSFER_OUT", None, None, today()));
    assert!(active_on("ENROLLED", None, None, today()));
}

#[test]
fn 아직_오지_않은_이동만_예정으로_본다() {
    let asof = d(2026, 10, 1);
    assert_eq!(
        pending_on("TRANSFER_IN", Some("2026-10-05"), None, asof),
        Some(Pending::In(d(2026, 10, 5)))
    );
    assert_eq!(
        pending_on("TRANSFER_OUT", None, Some("2026-10-10"), asof),
        Some(Pending::Out(d(2026, 10, 10)))
    );
    // 당일과 지난 날은 예정이 아니다 — 이미 일어난 일이다
    assert_eq!(pending_on("TRANSFER_IN", Some("2026-10-01"), None, asof), None);
    assert_eq!(pending_on("TRANSFER_OUT", None, Some("2026-09-20"), asof), None);
    assert_eq!(pending_on("ENROLLED", None, None, asof), None);

    let p = Pending::Out(d(2026, 10, 10));
    assert_eq!(p.label(), "전출 예정");
    assert_eq!(p.code(), "OUT");
    assert_eq!(p.date(), d(2026, 10, 10));
    assert_eq!(Pending::In(d(2026, 10, 5)).label(), "전입 예정");
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
    assert_eq!(check_date("2026-09-10", 2026, None), Ok(d(2026, 9, 10)));
    assert_eq!(check_date("2026-03-02", 2026, None), Ok(d(2026, 3, 2)));
    assert_eq!(check_date("2026-09-15", 2026, None), Ok(today()), "오늘도 받는다");
}

#[test]
fn 날짜로_읽지_못하면_거절한다() {
    for bad in ["", "어제", "2026/09/10", "20260910", "2026-13-40"] {
        assert_eq!(check_date(bad, 2026, None), Err(DateProblem::Unreadable), "{bad}");
    }
}

#[test]
fn 아직_오지_않은_날도_받는다() {
    // 학교는 전입·전출 예정일을 미리 알고 며칠 전에 적어 둔다
    assert_eq!(check_date("2026-09-16", 2026, None), Ok(d(2026, 9, 16)));
    assert_eq!(check_date("2027-02-28", 2026, None), Ok(d(2027, 2, 28)));
}

#[test]
fn 학년도_밖의_날짜는_거절한다() {
    // 2026학년도는 2026-03-01 부터다
    assert_eq!(check_date("2026-02-28", 2026, None), Err(DateProblem::OutOfYear));
    // 지난 학년도 날짜도 마찬가지
    assert_eq!(check_date("2025-09-10", 2026, None), Err(DateProblem::OutOfYear));
    // 앞으로도 학년도를 넘기지는 못한다 — 다음 해 학적은 학년도 전환이 만든다
    assert_eq!(check_date("2027-03-01", 2026, None), Err(DateProblem::OutOfYear));
}

#[test]
fn 앞선_이동보다_이른_날짜는_거절한다() {
    let last = ("전출", d(2026, 5, 14));
    let err = check_date("2026-04-01", 2026, Some(last)).unwrap_err();
    assert_eq!(
        err,
        DateProblem::BeforeLast {
            last_label: "전출".into(),
            last_date: d(2026, 5, 14)
        }
    );

    // 같은 날은 받는다 — 하루에 나갔다 들어오는 일이 없지는 않다
    assert!(check_date("2026-05-14", 2026, Some(last)).is_ok());
    assert!(check_date("2026-09-10", 2026, Some(last)).is_ok());
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
