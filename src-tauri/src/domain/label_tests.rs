//! 학년·반 표시와 정렬 검사.

use super::*;

#[test]
fn 글자_반과_숫자_반을_모두_표시한다() {
    assert_eq!(class_label(1, Some("나리")), "1-나리");
    assert_eq!(class_label(3, Some("1")), "3-1");
    assert_eq!(class_label(6, Some("가람")), "6-가람");
}

#[test]
fn 반이_없으면_미정으로_보여준다() {
    assert_eq!(class_label(2, None), "2-미정");
    assert_eq!(class_label(2, Some("")), "2-미정");
    assert_eq!(class_label(2, Some("   ")), "2-미정");
}

#[test]
fn 형제_표시는_요구사항_형식과_같다() {
    assert_eq!(student_label(1, Some("나리"), "홍길동"), "1-나리 홍길동");
}

#[test]
fn 숫자_반이_글자_반보다_앞선다() {
    let num = class_sort_key(Some("2"));
    let text = class_sort_key(Some("가람"));
    assert!(num < text);
}

#[test]
fn 숫자_반은_숫자_순서로_늘어선다() {
    let mut v = vec![Some("10"), Some("2"), Some("1"), Some("11")];
    v.sort_by_key(|c| class_sort_key(*c));
    assert_eq!(v, vec![Some("1"), Some("2"), Some("10"), Some("11")]);
}

#[test]
fn 글자_반은_가나다_순서로_늘어선다() {
    let mut v = vec![Some("다솜"), Some("가람"), Some("라온"), Some("나리")];
    v.sort_by_key(|c| class_sort_key(*c));
    assert_eq!(
        v,
        vec![Some("가람"), Some("나리"), Some("다솜"), Some("라온")]
    );
}

#[test]
fn 반이_없는_학생은_맨_뒤로_간다() {
    let mut v = vec![None, Some("가람"), Some("1")];
    v.sort_by_key(|c| class_sort_key(*c));
    assert_eq!(v, vec![Some("1"), Some("가람"), None]);
}

#[test]
fn 섞여_있어도_숫자_먼저_그다음_가나다() {
    let mut v = vec![Some("나리"), Some("3"), Some("가람"), Some("1"), None];
    v.sort_by_key(|c| class_sort_key(*c));
    assert_eq!(
        v,
        vec![Some("1"), Some("3"), Some("가람"), Some("나리"), None]
    );
}

#[test]
fn 명단_정렬은_반_정렬_규칙을_그대로_품는다() {
    assert!(
        ORDER_BY_ROSTER.contains(ORDER_BY_CLASS),
        "명단과 통계가 반을 다른 차례로 늘어놓으면 같은 자료가 다른 표처럼 읽힌다"
    );
    assert!(ORDER_BY_ROSTER.starts_with("e.grade"), "학년이 맨 앞이다");
    assert!(ORDER_BY_ROSTER.ends_with("s.name"), "마지막은 이름이다");
}
