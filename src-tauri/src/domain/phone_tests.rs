//! 연락처 정규화 검사.

use super::*;

#[test]
fn 하이픈이_있든_없든_같은_숫자가_된다() {
    for raw in [
        "010-1234-5678",
        "01012345678",
        "010 1234 5678",
        "010.1234.5678",
        " 010-1234-5678 ",
    ] {
        let (display, d) = normalize(raw);
        assert_eq!(d.as_deref(), Some("01012345678"), "{raw}");
        assert_eq!(
            display.as_deref(),
            Some("010-1234-5678"),
            "{raw} 는 보기 좋은 형태로 저장한다"
        );
    }
}

#[test]
fn 열한자리_휴대전화를_읽는다() {
    assert_eq!(
        normalize("01098765432").0.as_deref(),
        Some("010-9876-5432")
    );
}

#[test]
fn 열자리_옛_휴대전화도_읽는다() {
    assert_eq!(normalize("0111234567").0.as_deref(), Some("011-123-4567"));
}

#[test]
fn 지역번호도_보기_좋게_만든다() {
    assert_eq!(normalize("021234567").0.as_deref(), Some("02-123-4567"));
    assert_eq!(normalize("0212345678").0.as_deref(), Some("02-1234-5678"));
    assert_eq!(normalize("0441234567").0.as_deref(), Some("044-123-4567"));
}

#[test]
fn 모양이_이상해도_저장을_막지_않는다() {
    // 자릿수가 모자란 값 — 원문을 그대로 두고 숫자만 뽑아 둔다
    let (display, d) = normalize("1234");
    assert_eq!(display.as_deref(), Some("1234"));
    assert_eq!(d.as_deref(), Some("1234"));

    // 내선·안내 문구가 섞인 값
    let (display, d) = normalize("010-1234-5678 (엄마)");
    assert_eq!(d.as_deref(), Some("01012345678"));
    assert!(display.is_some(), "무엇이든 남겨 둔다");
}

#[test]
fn 숫자가_하나도_없으면_원문만_남는다() {
    let (display, d) = normalize("없음");
    assert_eq!(display.as_deref(), Some("없음"));
    assert_eq!(d, None);
}

#[test]
fn 빈_값은_둘_다_비운다() {
    for raw in ["", "   ", "\t\n"] {
        assert_eq!(normalize(raw), (None, None), "{raw:?}");
    }
}

#[test]
fn 숫자만_뽑아낸다() {
    assert_eq!(digits("010-1234-5678"), "01012345678");
    assert_eq!(digits("가나다"), "");
    assert_eq!(digits(""), "");
}

// ---------- 부분 검색 ----------

#[test]
fn 가운데_네_자리와_끝_네_자리로_모두_찾을_수_있다() {
    let stored = normalize("010-1234-5678").1.unwrap();
    for term in ["1234", "5678", "01012345678", "010-1234", "2345"] {
        let needle = search_digits(term, 3).expect(term);
        assert!(
            stored.contains(&needle),
            "{term} 으로 {stored} 를 찾지 못한다"
        );
    }
}

#[test]
fn 너무_짧은_검색어는_연락처_검색으로_보지_않는다() {
    assert_eq!(search_digits("7", 3), None);
    assert_eq!(search_digits("12", 3), None);
    assert_eq!(search_digits("123", 3).as_deref(), Some("123"));
}

#[test]
fn 검색어에_섞인_하이픈은_무시한다() {
    assert_eq!(search_digits("010-1234", 3).as_deref(), Some("0101234"));
}

// ---------- 형제 비교 ----------

#[test]
fn 형제_비교에는_온전한_번호만_쓴다() {
    assert!(is_comparable(&normalize("010-1234-5678").1));
    assert!(is_comparable(&normalize("02-123-4567").1));
    assert!(!is_comparable(&normalize("1234").1), "네 자리는 우연히 겹친다");
    assert!(!is_comparable(&None));
}
