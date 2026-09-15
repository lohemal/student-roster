//! 학년도 전환 규칙 검사.
//!
//! 여기서 보는 것 하나: **프로그램이 사람 대신 정하지 않는가.**
//! 반을 지어내지 않고, 반쯤 입력된 번호를 채우지 않고, 7학년을 만들지 않는다.
//! 이름은 모두 가상이다.

use super::*;

// ---------------------------------------------------------------
// 학년
// ---------------------------------------------------------------

#[test]
fn 일학년부터_오학년까지는_한_학년_올라간다() {
    for g in 1..=5 {
        assert_eq!(next_grade(g), Some(g + 1), "{g}학년");
        assert_eq!(outcome(g), Outcome::Promote(g + 1));
    }
}

#[test]
fn 육학년은_다음_학년이_없고_졸업이다() {
    assert_eq!(next_grade(6), None, "7학년을 만들지 않는다");
    assert_eq!(outcome(6), Outcome::Graduate);
}

#[test]
fn 학년_범위를_벗어난_값은_받지_않는다() {
    assert!(is_valid_grade(1));
    assert!(is_valid_grade(6));
    assert!(!is_valid_grade(0));
    assert!(!is_valid_grade(7));
    assert!(!is_valid_grade(-1));
}

// ---------------------------------------------------------------
// 번호
// ---------------------------------------------------------------

#[test]
fn 번호가_전부_비었는지_전부_있는지_가려낸다() {
    assert_eq!(number_state(&[None, None, None]), NumberState::AllEmpty);
    assert_eq!(
        number_state(&[Some(1), Some(2), Some(3)]),
        NumberState::AllGiven
    );
    assert_eq!(number_state(&[]), NumberState::AllEmpty, "빈 반");
}

#[test]
fn 일부만_입력된_번호는_프로그램이_채우지_않는다() {
    // 홍길동 1 · 김민준 없음 · 이서준 5 · 박지우 없음
    let state = number_state(&[Some(1), None, Some(5), None]);
    assert_eq!(
        state,
        NumberState::Partial,
        "빈 자리를 채우면 사람이 의도한 배치가 깨진다"
    );
}

#[test]
fn 번호가_없으면_이름_가나다순으로_매긴다() {
    let names = vec![
        (10, "한가람".to_string()),
        (11, "가온해".to_string()),
        (12, "나린별".to_string()),
    ];
    assert_eq!(numbers_by_name(&names), vec![(11, 1), (12, 2), (10, 3)]);
}

#[test]
fn 이름이_같으면_들어온_차례를_지킨다() {
    let names = vec![
        (7, "가온해".to_string()),
        (3, "가온해".to_string()),
        (9, "나린별".to_string()),
    ];
    assert_eq!(numbers_by_name(&names), vec![(7, 1), (3, 2), (9, 3)]);
}

#[test]
fn 이름_차례는_명단_정렬과_같은_규칙이다() {
    // SQLite 의 ORDER BY 도 UTF-8 바이트 차례이고 한글은 그것이 곧 가나다 차례다
    let mut a = vec!["하늬", "가람", "나리", "다솜", "라온", "마루", "바다", "사랑"];
    a.sort();
    assert_eq!(
        a,
        vec!["가람", "나리", "다솜", "라온", "마루", "바다", "사랑", "하늬"]
    );
}

#[test]
fn 같은_번호가_둘이면_찾아낸다() {
    assert_eq!(duplicate_numbers(&[Some(1), Some(2), Some(1)]), vec![1]);
    assert_eq!(
        duplicate_numbers(&[Some(3), Some(3), Some(7), Some(7)]),
        vec![3, 7]
    );
    assert!(duplicate_numbers(&[Some(1), Some(2), None, None]).is_empty());
}

// ---------------------------------------------------------------
// 막는 것과 알리는 것
// ---------------------------------------------------------------

#[test]
fn 학적을_만들_수_없는_문제만_전환을_막는다() {
    for k in [
        ProblemKind::NoAssign,
        ProblemKind::NoClass,
        ProblemKind::Duplicated,
        ProblemKind::UnknownStudent,
        ProblemKind::NameMismatch,
        ProblemKind::BadGrade,
        ProblemKind::PartialNumbers,
        ProblemKind::NumberDup,
        ProblemKind::WrongYear,
        ProblemKind::TargetNotEmpty,
        ProblemKind::AlreadyDone,
    ] {
        assert!(k.blocking(), "{}", k.label());
    }
    for k in [
        ProblemKind::StaleSeat,
        ProblemKind::UnusualGrade,
        ProblemKind::NoNextEnrollment,
    ] {
        assert!(!k.blocking(), "{} 은 알리기만 한다", k.label());
    }
}

// ---------------------------------------------------------------
// 원본이 그대로인가
// ---------------------------------------------------------------

fn seat(id: i64, grade: i32, class_name: &str, no: i32) -> Seat {
    Seat {
        student_id: id,
        grade,
        class_name: Some(class_name.into()),
        class_no: Some(no),
        status: "ENROLLED".into(),
    }
}

#[test]
fn 같은_자리면_같은_열쇠다() {
    let a = vec![seat(1, 3, "가람", 7), seat(2, 3, "가람", 8)];
    let b = vec![seat(2, 3, "가람", 8), seat(1, 3, "가람", 7)];
    assert_eq!(state_key(&a), state_key(&b), "차례가 달라도 같은 상태다");
}

#[test]
fn 학적이_바뀌면_열쇠가_달라진다() {
    let base = vec![seat(1, 3, "가람", 7), seat(2, 3, "가람", 8)];
    let key = state_key(&base);

    let mut moved = base.clone();
    moved[0].class_no = Some(9);
    assert_ne!(state_key(&moved), key, "번호가 바뀌었다");

    let mut other_class = base.clone();
    other_class[0].class_name = Some("나리".into());
    assert_ne!(state_key(&other_class), key, "반이 바뀌었다");

    let mut left = base.clone();
    left[0].status = "TRANSFER_OUT".into();
    assert_ne!(state_key(&left), key, "전출했다");

    let mut fewer = base.clone();
    fewer.pop();
    assert_ne!(state_key(&fewer), key, "학생이 줄었다");
}
