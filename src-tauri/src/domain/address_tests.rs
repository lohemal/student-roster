//! 주소 뜯어보기·판정 검사.
//!
//! 주소를 잘못 읽으면 통계가 통째로 틀어진다. 특히 **비슷하지만 다른 건물**을
//! 같은 것으로 보지 않는지 꼼꼼히 본다. 시험 주소는 모두 가상이다.

use super::*;

fn cat(id: i64, name: &str) -> CategoryRef {
    CategoryRef {
        id,
        name: name.to_string(),
    }
}

fn rule(id: i64, kind: RuleKind, pattern: &str, category_id: i64, name: &str) -> RuleRef {
    RuleRef {
        id,
        kind,
        pattern: pattern.to_string(),
        category_id,
        category_name: name.to_string(),
    }
}

// ---------------------------------------------------------------
// 정규화
// ---------------------------------------------------------------

#[test]
fn 이어진_공백을_하나로_줄인다() {
    assert_eq!(normalize("○○로   123"), "○○로 123");
    assert_eq!(normalize("  ○○로 123  "), "○○로 123");
    assert_eq!(normalize("○○로\t123\n"), "○○로 123");
}

#[test]
fn 전각_숫자와_괄호를_반각으로_바꾼다() {
    assert_eq!(normalize("○○로 １２３"), "○○로 123");
    assert_eq!(normalize("○○로 123（5단지）"), "○○로 123(5단지)");
    assert_eq!(normalize("○○로　123"), "○○로 123");
}

#[test]
fn 원본을_지나치게_바꾸지_않는다() {
    // 사람이 읽을 수 있어야 하고, 검색에도 쓰인다
    let n = normalize("○○시 ○○로 123, 101동 1001호(○○마을5단지)");
    assert_eq!(n, "○○시 ○○로 123, 101동 1001호(○○마을5단지)");
}

#[test]
fn 빈_주소는_빈_결과다() {
    let p = parse("");
    assert_eq!(p.norm, "");
    assert_eq!(p.road, None);
    assert!(p.complexes.is_empty());

    assert_eq!(parse("   ").road, None);
}

// ---------------------------------------------------------------
// 도로명 + 건물번호 뽑기
// ---------------------------------------------------------------

#[test]
fn 도로명과_건물번호를_뽑는다() {
    for raw in [
        "○○로 123",
        "○○로123",
        "○○시 ○○로 123",
        "○○로 123, 101동 1001호",
        "○○로 123 105동 804호",
        "○○로 123(○○마을5단지)",
        "○○로 123 (○○마을 5단지)",
        "○○시 ○○동 ○○로 123, 110동 1502호(○○마을5단지)",
    ] {
        assert_eq!(
            parse(raw).road.as_deref(),
            Some("○○로 123"),
            "주소: {raw}"
        );
    }
}

#[test]
fn 동과_호수는_도로로_보지_않는다() {
    // 101동 1001호 는 로·길로 끝나지 않는다
    let p = parse("○○로 123, 101동 1001호");
    assert_eq!(p.road.as_deref(), Some("○○로 123"));

    // 도로가 아예 없으면 뽑지 못한다
    assert_eq!(parse("101동 1001호").road, None);
    assert_eq!(parse("○○아파트 3동 502호").road, None);
}

#[test]
fn 숫자가_비슷한_다른_건물을_같게_보지_않는다() {
    let a = parse("○○로 12").road;
    let b = parse("○○로 123").road;
    let c = parse("○○로 1234").road;
    assert_eq!(a.as_deref(), Some("○○로 12"));
    assert_eq!(b.as_deref(), Some("○○로 123"));
    assert_eq!(c.as_deref(), Some("○○로 1234"));
    assert_ne!(a, b);
    assert_ne!(b, c);
}

#[test]
fn 대로와_길도_알아본다() {
    assert_eq!(parse("○○대로 2130").road.as_deref(), Some("○○대로 2130"));
    assert_eq!(parse("○○길 45").road.as_deref(), Some("○○길 45"));
}

#[test]
fn 로와_길이_겹치면_더_좁은_길을_쓴다() {
    // '○○로3길 25' 에서 '○○로 3' 을 뽑으면 엉뚱한 건물이 된다
    assert_eq!(parse("○○로3길 25").road.as_deref(), Some("○○로3길 25"));
    assert_eq!(
        parse("○○시 ○○로3길 25, 2동 301호").road.as_deref(),
        Some("○○로3길 25")
    );
}

#[test]
fn 번길이_떨어져_있으면_앞_도로명을_붙인다() {
    // '123번길' 만으로는 어느 길인지 알 수 없다
    assert_eq!(
        parse("○○로 123번길 45").road.as_deref(),
        Some("○○로123번길 45")
    );
}

#[test]
fn 가지번호가_있는_건물번호도_읽는다() {
    assert_eq!(parse("○○로 123-4").road.as_deref(), Some("○○로 123-4"));
    assert_ne!(parse("○○로 123-4").road, parse("○○로 123").road);
}

#[test]
fn 번호가_없으면_도로를_뽑지_않는다() {
    assert_eq!(parse("○○로").road, None);
    assert_eq!(parse("○○시 ○○로 ○○아파트").road, None);
}

#[test]
fn 같은_건물의_여러_적음이_같은_값이_된다() {
    let forms = [
        "○○로 123, 101동 1001호",
        "○○로123 105동 804호",
        "○○로  123 (○○마을5단지)",
        "○○시 ○○로 123, 110동 1502호",
    ];
    let roads: Vec<_> = forms.iter().map(|f| parse(f).road).collect();
    assert!(
        roads.iter().all(|r| *r == roads[0]),
        "같은 건물인데 다르게 읽혔다: {roads:?}"
    );
}

// ---------------------------------------------------------------
// 단지 이름 뽑기
// ---------------------------------------------------------------

#[test]
fn 단지_이름을_긴_것부터_뽑는다() {
    let p = parse("○○로 123, 101동 1001호(가온마을5단지)");
    assert_eq!(p.complexes, vec!["가온마을5단지", "5단지"]);
}

#[test]
fn 띄어_쓴_단지_이름도_뽑는다() {
    let p = parse("○○로 123 (가온마을 5단지)");
    assert_eq!(p.complexes, vec!["가온마을5단지", "5단지"]);
}

#[test]
fn 단지라는_말이_없으면_뽑지_않는다() {
    // 건물번호 5 를 5단지로 읽으면 안 된다
    assert!(parse("○○로 5").complexes.is_empty());
    assert!(parse("○○로 123, 5동 501호").complexes.is_empty());
    assert!(parse("○○아파트 5").complexes.is_empty());
}

#[test]
fn 단지가_둘이면_둘_다_뽑는다() {
    let p = parse("○○로 123(가온마을5단지), 나온마을3단지 앞");
    assert!(p.complexes.contains(&"가온마을5단지".to_string()));
    assert!(p.complexes.contains(&"나온마을3단지".to_string()));
}

#[test]
fn 숫자가_없는_단지는_뽑지_않는다() {
    assert!(parse("○○단지").complexes.is_empty());
}

// ---------------------------------------------------------------
// ROAD 규칙
// ---------------------------------------------------------------

#[test]
fn 도로명_규칙은_동_호수가_달라도_걸린다() {
    for raw in [
        "○○로 123, 101동 1001호",
        "○○로 123 105동 804호",
        "○○로123, 110동 1502호",
        "○○로 123(○○마을5단지)",
    ] {
        assert!(
            matches(RuleKind::Road, "○○로 123", &parse(raw)),
            "주소: {raw}"
        );
    }
}

#[test]
fn 도로명_규칙은_번호가_다르면_걸리지_않는다() {
    let p12 = parse("○○로 12, 101동 1001호");
    let p123 = parse("○○로 123, 101동 1001호");
    let p1234 = parse("○○로 1234, 101동 1001호");

    assert!(!matches(RuleKind::Road, "○○로 123", &p12), "12 가 123 에 걸림");
    assert!(!matches(RuleKind::Road, "○○로 123", &p1234), "1234 가 123 에 걸림");
    assert!(matches(RuleKind::Road, "○○로 123", &p123));

    // 반대 방향도
    assert!(!matches(RuleKind::Road, "○○로 12", &p123));
    assert!(!matches(RuleKind::Road, "○○로 1234", &p123));
}

#[test]
fn 도로명이_다르면_번호가_같아도_걸리지_않는다() {
    let p = parse("△△로 123, 101동 1001호");
    assert!(!matches(RuleKind::Road, "○○로 123", &p));
}

#[test]
fn 규칙_패턴에_주소를_통째로_넣어도_도로만_남는다() {
    assert_eq!(
        normalize_pattern(RuleKind::Road, "○○시 ○○로 123, 101동 1001호"),
        "○○로 123"
    );
    // 그렇게 저장된 규칙이 제대로 걸린다
    let p = parse("○○로 123, 999동 1호");
    assert!(matches(RuleKind::Road, "○○시 ○○로 123, 101동 1001호", &p));
}

#[test]
fn 도로를_뽑지_못한_주소는_도로명_규칙에_걸리지_않는다() {
    let p = parse("○○아파트 101동 1001호");
    assert_eq!(p.road, None);
    assert!(!matches(RuleKind::Road, "○○로 123", &p));
}

// ---------------------------------------------------------------
// COMPLEX 규칙
// ---------------------------------------------------------------

#[test]
fn 단지명_규칙은_띄어쓰기를_무시한다() {
    let forms = [
        "○○로 123(가온마을5단지)",
        "○○로 123 (가온마을 5단지)",
        "○○로 123, 101동 1001호 가온마을5단지",
    ];
    for raw in forms {
        assert!(
            matches(RuleKind::Complex, "가온마을5단지", &parse(raw)),
            "주소: {raw}"
        );
        assert!(
            matches(RuleKind::Complex, "가온마을 5단지", &parse(raw)),
            "규칙에 띄어쓰기가 있어도: {raw}"
        );
    }
}

#[test]
fn 비슷하지만_다른_단지는_걸리지_않는다() {
    let p = parse("○○로 123(가온마을5단지)");
    assert!(!matches(RuleKind::Complex, "가온마을15단지", &p));
    assert!(!matches(RuleKind::Complex, "나온마을5단지", &p));
}

#[test]
fn 단지_번호가_다르면_걸리지_않는다() {
    let p = parse("○○로 123(가온마을3단지)");
    assert!(!matches(RuleKind::Complex, "가온마을5단지", &p));
}

// ---------------------------------------------------------------
// CONTAINS 규칙
// ---------------------------------------------------------------

#[test]
fn 포함_규칙은_글자가_들어_있으면_걸린다() {
    let p = parse("○○로 123 가온빌라 201호");
    assert!(matches(RuleKind::Contains, "가온빌라", &p));
    assert!(matches(RuleKind::Contains, "가온 빌라", &p), "띄어쓰기 무시");
    assert!(!matches(RuleKind::Contains, "나온빌라", &p));
}

#[test]
fn 빈_규칙은_아무것도_걸지_않는다() {
    let p = parse("○○로 123");
    assert!(!matches(RuleKind::Contains, "", &p));
    assert!(!matches(RuleKind::Contains, "   ", &p));
    assert!(!matches(RuleKind::Road, "", &p));
}

// ---------------------------------------------------------------
// 우선순위
// ---------------------------------------------------------------

#[test]
fn 도로명_규칙이_포함_규칙보다_앞선다() {
    let cats = vec![cat(1, "3단지"), cat(2, "5단지")];
    let rules = vec![
        rule(10, RuleKind::Contains, "○○마을", 1, "3단지"),
        rule(20, RuleKind::Road, "○○로 123", 2, "5단지"),
    ];
    let p = parse("○○로 123, 101동 1001호(○○마을)");

    match classify(&p, &rules, &cats) {
        Decision::Rule { category_id, hit } => {
            assert_eq!(category_id, 2, "도로명 규칙(5단지)이 이겨야 한다");
            assert_eq!(hit.rule_id, 20);
        }
        other => panic!("도로명 규칙으로 정해야 한다: {other:?}"),
    }
}

#[test]
fn 단지명_규칙이_포함_규칙보다_앞선다() {
    let cats = vec![cat(1, "주택"), cat(2, "5단지")];
    let rules = vec![
        rule(10, RuleKind::Contains, "○○시", 1, "주택"),
        rule(20, RuleKind::Complex, "가온마을5단지", 2, "5단지"),
    ];
    let p = parse("○○시 ○○로 123(가온마을5단지)");
    assert_eq!(classify(&p, &rules, &cats).category_id(), Some(2));
}

#[test]
fn 규칙이_있으면_주소의_단지_이름보다_규칙을_따른다() {
    // 주소에는 5단지라고 적혀 있지만 학교가 3단지로 정한 경우
    let cats = vec![cat(1, "3단지"), cat(2, "5단지")];
    let rules = vec![rule(10, RuleKind::Road, "○○로 123", 1, "3단지")];
    let p = parse("○○로 123(가온마을5단지)");

    let d = classify(&p, &rules, &cats);
    assert_eq!(d.category_id(), Some(1));
    assert_eq!(d.source(), "RULE");
}

// ---------------------------------------------------------------
// 충돌
// ---------------------------------------------------------------

#[test]
fn 같은_순위_규칙이_다른_분류를_가리키면_정하지_않는다() {
    let cats = vec![cat(1, "3단지"), cat(2, "5단지")];
    let rules = vec![
        rule(10, RuleKind::Road, "○○로 123", 1, "3단지"),
        rule(20, RuleKind::Road, "○○시 ○○로 123", 2, "5단지"), // 같은 도로로 정규화된다
    ];
    let p = parse("○○로 123, 101동 1001호");

    match classify(&p, &rules, &cats) {
        Decision::Conflict { hits } => {
            assert_eq!(hits.len(), 2);
            assert_eq!(classify(&p, &rules, &cats).category_id(), None);
            assert_eq!(classify(&p, &rules, &cats).source(), "CONFLICT");
            // 무엇이 부딪혔는지 알 수 있어야 한다
            let names: Vec<&str> = hits.iter().map(|h| h.category_name.as_str()).collect();
            assert!(names.contains(&"3단지") && names.contains(&"5단지"));
        }
        other => panic!("충돌로 남겨야 한다: {other:?}"),
    }
}

#[test]
fn 충돌_사유에_부딪힌_분류가_보인다() {
    let cats = vec![cat(1, "3단지"), cat(2, "5단지")];
    let rules = vec![
        rule(10, RuleKind::Contains, "○○마을", 1, "3단지"),
        rule(20, RuleKind::Contains, "○○로", 2, "5단지"),
    ];
    let d = classify(&parse("○○로 123 ○○마을"), &rules, &cats);
    let reason = d.reason();
    assert!(reason.contains("3단지"), "{reason}");
    assert!(reason.contains("5단지"), "{reason}");
}

#[test]
fn 같은_순위_규칙이_같은_분류면_충돌이_아니다() {
    let cats = vec![cat(1, "5단지")];
    let rules = vec![
        rule(10, RuleKind::Contains, "○○마을", 1, "5단지"),
        rule(20, RuleKind::Contains, "○○로", 1, "5단지"),
    ];
    let d = classify(&parse("○○로 123 ○○마을"), &rules, &cats);
    assert_eq!(d.category_id(), Some(1));
    assert_eq!(d.source(), "RULE");
}

#[test]
fn 낮은_순위의_충돌은_높은_순위가_정해지면_문제가_되지_않는다() {
    let cats = vec![cat(1, "3단지"), cat(2, "5단지"), cat(3, "주택")];
    let rules = vec![
        rule(10, RuleKind::Road, "○○로 123", 3, "주택"),
        // 아래 둘은 서로 부딪히지만 도로명 규칙이 먼저 정해진다
        rule(20, RuleKind::Contains, "○○마을", 1, "3단지"),
        rule(30, RuleKind::Contains, "○○로", 2, "5단지"),
    ];
    let d = classify(&parse("○○로 123 ○○마을"), &rules, &cats);
    assert_eq!(d.category_id(), Some(3));
}

// ---------------------------------------------------------------
// 주소의 단지 이름으로 자동 판정
// ---------------------------------------------------------------

#[test]
fn 규칙이_없어도_주소의_단지_이름으로_정한다() {
    let cats = vec![cat(1, "5단지"), cat(2, "주택")];
    let d = classify(&parse("○○로 123(가온마을5단지)"), &[], &cats);
    match d {
        Decision::Auto { category_id, token } => {
            assert_eq!(category_id, 1);
            assert_eq!(token, "5단지");
        }
        other => panic!("자동 분류되어야 한다: {other:?}"),
    }
}

#[test]
fn 긴_단지_이름이_있으면_그것을_먼저_쓴다() {
    let cats = vec![cat(1, "5단지"), cat(2, "가온마을5단지")];
    let d = classify(&parse("○○로 123(가온마을5단지)"), &[], &cats);
    assert_eq!(
        d.category_id(),
        Some(2),
        "짧은 '5단지' 보다 '가온마을5단지' 가 더 정확하다"
    );
}

#[test]
fn 해당_분류가_없으면_만들지_않는다() {
    let cats = vec![cat(1, "주택"), cat(2, "기타")];
    let d = classify(&parse("○○로 123(가온마을5단지)"), &[], &cats);
    assert_eq!(d, Decision::Unclassified, "없는 분류를 만들어 내면 안 된다");
}

#[test]
fn 단지가_둘이면_저절로_정하지_않는다() {
    let cats = vec![cat(1, "3단지"), cat(2, "5단지")];
    let d = classify(&parse("○○로 123(가온마을5단지), 나온마을3단지 앞"), &[], &cats);
    assert_eq!(d, Decision::Unclassified, "어느 쪽인지 알 수 없다");
}

#[test]
fn 건물번호를_단지로_잘못_읽지_않는다() {
    let cats = vec![cat(1, "5단지"), cat(2, "123단지")];
    let d = classify(&parse("○○로 123, 5동 501호"), &[], &cats);
    assert_eq!(d, Decision::Unclassified);
}

// ---------------------------------------------------------------
// 미분류
// ---------------------------------------------------------------

#[test]
fn 걸리는_것이_없으면_미분류다() {
    let cats = vec![cat(1, "5단지")];
    let d = classify(&parse("○○로 999, 1동 101호"), &[], &cats);
    assert_eq!(d, Decision::Unclassified);
    assert_eq!(d.category_id(), None);
    assert_eq!(d.source(), "NONE");
}

#[test]
fn 주소가_없으면_주소_없음이다() {
    let d = classify(&parse(""), &[], &[]);
    assert_eq!(d, Decision::NoAddress);
    assert_eq!(d.source(), "NONE");
    assert!(d.reason().contains("주소가 없"));
}

#[test]
fn 미분류와_기타는_다르다() {
    // '기타' 는 사용자가 기타라고 정한 분류다. 미분류를 기타로 밀어 넣지 않는다.
    let cats = vec![cat(1, "기타")];
    let d = classify(&parse("○○로 999"), &[], &cats);
    assert_eq!(d, Decision::Unclassified);
    assert_ne!(d.category_id(), Some(1));
}

// ---------------------------------------------------------------
// 규칙 제안
// ---------------------------------------------------------------

#[test]
fn 주소에서_규칙_후보를_내놓는다() {
    let p = parse("○○시 ○○로 123, 101동 1001호(가온마을5단지)");
    let s = suggest(&p);

    assert_eq!(s[0].kind, "ROAD", "도로명을 가장 먼저 권한다");
    assert_eq!(s[0].pattern, "○○로 123");
    assert!(s.iter().any(|x| x.kind == "COMPLEX" && x.pattern == "가온마을5단지"));
    assert!(!s[0].hint.is_empty());
}

#[test]
fn 동_호수는_규칙_후보에_들어가지_않는다() {
    let s = suggest(&parse("○○로 123, 101동 1001호"));
    assert!(
        s.iter().all(|x| !x.pattern.contains('동') && !x.pattern.contains('호')),
        "{s:?}"
    );
}

#[test]
fn 뽑을_것이_없으면_제안도_없다() {
    assert!(suggest(&parse("")).is_empty());
    assert!(suggest(&parse("어딘가")).is_empty());
}

// ---------------------------------------------------------------
// 종류별 안내
// ---------------------------------------------------------------

#[test]
fn 규칙_종류마다_사람이_읽을_설명이_있다() {
    for k in [RuleKind::Road, RuleKind::Complex, RuleKind::Contains] {
        assert!(!k.label().is_empty());
        assert!(k.hint().len() > 10, "{}", k.code());
        assert_eq!(RuleKind::parse(k.code()), Some(k));
    }
    assert_eq!(RuleKind::parse("무엇인가"), None);
}

#[test]
fn 포함_규칙_설명은_넓게_걸린다는_것을_알려_준다() {
    assert!(RuleKind::Contains.hint().contains("넓게"));
}
