//! 주소 분류 저장소 검사.
//!
//! 학생을 실제로 넣고, 규칙을 만들고, 다시 적용하며 확인한다.
//! 특히 **직접 지정한 분류가 자동 재적용에 살아남는지**를 꼼꼼히 본다.

use super::*;
use crate::db::Db;
use crate::repo::{issue, settings, student};
use chrono::NaiveDate;

fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 9, 15).unwrap()
}

fn db() -> Db {
    let db = Db::memory();
    db.write(|c| {
        settings::create_year(c, 2026)?;
        settings::set_current_year(c, 2026)
    })
    .unwrap();
    db
}

/// 학생 하나를 주소와 함께 넣는다.
fn add(db: &Db, name: &str, addr: &str) -> i64 {
    let input = student::StudentInput {
        name: name.into(),
        gender: Some("M".into()),
        birth_raw: Some("170315".into()),
        address_raw: (!addr.is_empty()).then(|| addr.to_string()),
        primary_phone: Some("010-1234-5678".into()),
        school_year: 2026,
        grade: 3,
        class_name: Some("가람".into()),
        class_no: Some(1),
        ..Default::default()
    };
    db.write(|c| student::create(c, &input, today())).unwrap()
}

fn category(db: &Db, name: &str) -> i64 {
    db.write(|c| create_category(c, name)).unwrap()
}

fn add_rule(db: &Db, kind: &str, pattern: &str, cat: i64) -> i64 {
    db.write(|c| create_rule(c, kind, pattern, cat, None)).unwrap()
}

/// 학생의 (분류 이름, 판정 근거)
fn cat_of(db: &Db, id: i64) -> (Option<String>, String) {
    db.read(|c| {
        Ok(c.query_row(
            "SELECT ac.name, s.address_source
               FROM students s LEFT JOIN address_categories ac ON ac.id = s.address_category_id
              WHERE s.id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?)
    })
    .unwrap()
}

fn has_address_issue(db: &Db, id: i64) -> bool {
    db.read(|c| Ok(issue::list_for_student(c, id)?))
        .unwrap()
        .iter()
        .any(|i| i.kind == "ADDRESS")
}

// ---------------------------------------------------------------
// 기본 분류
// ---------------------------------------------------------------

#[test]
fn 기본_분류로_주택과_기타가_들어_있다() {
    let db = db();
    let cats = db.read(|c| list_categories(c, 2026, today())).unwrap();
    let names: Vec<&str> = cats.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, vec!["주택", "기타"]);
    assert!(cats.iter().all(|c| c.is_builtin));
}

#[test]
fn 분류를_만들고_이름을_고친다() {
    let db = db();
    let id = category(&db, "5단지");

    let cats = db.read(|c| list_categories(c, 2026, today())).unwrap();
    assert!(cats.iter().any(|c| c.name == "5단지" && !c.is_builtin));

    db.write(|c| rename_category(c, id, "가온마을5단지")).unwrap();
    let cats = db.read(|c| list_categories(c, 2026, today())).unwrap();
    assert!(cats.iter().any(|c| c.name == "가온마을5단지"));
}

#[test]
fn 같은_이름의_분류를_두_번_만들지_않는다() {
    let db = db();
    category(&db, "5단지");
    let err = db.write(|c| create_category(c, "5단지")).unwrap_err();
    assert_eq!(err.code, "DUPLICATE");
    assert!(err.user_message.contains("5단지"));
}

#[test]
fn 쓰고_있는_분류는_지우지_못한다() {
    let db = db();
    let cat = category(&db, "5단지");
    let sid = add(&db, "홍길동", "○○로 123");
    db.write(|c| set_manual(c, sid, Some(cat))).unwrap();

    let err = db.write(|c| delete_category(c, cat)).unwrap_err();
    assert_eq!(err.code, "IN_USE");
    assert!(err.user_message.contains("학생 1명"), "{}", err.user_message);
}

#[test]
fn 규칙이_가리키는_분류도_지우지_못한다() {
    let db = db();
    let cat = category(&db, "5단지");
    add_rule(&db, "ROAD", "○○로 123", cat);

    let err = db.write(|c| delete_category(c, cat)).unwrap_err();
    assert_eq!(err.code, "IN_USE");
    assert!(err.user_message.contains("주소 규칙 1개"), "{}", err.user_message);
}

#[test]
fn 아무도_쓰지_않는_분류는_지울_수_있다() {
    let db = db();
    let cat = category(&db, "안쓰는분류");
    db.write(|c| delete_category(c, cat)).unwrap();
    let cats = db.read(|c| list_categories(c, 2026, today())).unwrap();
    assert!(!cats.iter().any(|c| c.name == "안쓰는분류"));
}

// ---------------------------------------------------------------
// 규칙 만들기
// ---------------------------------------------------------------

#[test]
fn 도로명_규칙은_주소를_통째로_넣어도_도로만_남는다() {
    let db = db();
    let cat = category(&db, "5단지");
    let rid = add_rule(&db, "ROAD", "○○시 ○○로 123, 101동 1001호", cat);

    let rules = db.read(|c| list_rules(c, 2026, today())).unwrap();
    let r = rules.iter().find(|r| r.id == rid).unwrap();
    assert_eq!(r.pattern, "○○로 123", "동·호수는 규칙에 들어가지 않는다");
    assert_eq!(r.kind_label, "도로명");
}

#[test]
fn 도로명이_없는_값으로는_도로명_규칙을_만들지_못한다() {
    let db = db();
    let cat = category(&db, "5단지");
    let err = db
        .write(|c| create_rule(c, "ROAD", "가온마을5단지", cat, None))
        .unwrap_err();
    assert_eq!(err.code, "INVALID_INPUT");
    assert!(err.user_message.contains("건물번호"));
}

#[test]
fn 너무_짧은_포함_규칙은_막는다() {
    let db = db();
    let cat = category(&db, "주택2");
    let err = db
        .write(|c| create_rule(c, "CONTAINS", "로", cat, None))
        .unwrap_err();
    assert_eq!(err.code, "INVALID_INPUT");
    assert!(err.user_message.contains("두 글자"));
}

#[test]
fn 없는_분류를_가리키는_규칙은_만들지_못한다() {
    let db = db();
    let err = db
        .write(|c| create_rule(c, "ROAD", "○○로 123", 9999, None))
        .unwrap_err();
    assert_eq!(err.code, "NOT_FOUND");
}

// ---------------------------------------------------------------
// 등록하자마자 분류
// ---------------------------------------------------------------

#[test]
fn 규칙이_있으면_학생을_넣자마자_분류된다() {
    let db = db();
    let cat = category(&db, "5단지");
    add_rule(&db, "ROAD", "○○로 123", cat);

    let sid = add(&db, "홍길동", "○○시 ○○로 123, 101동 1001호");
    assert_eq!(cat_of(&db, sid), (Some("5단지".into()), "RULE".into()));
    assert!(!has_address_issue(&db, sid), "분류됐으면 확인 필요가 없다");
}

#[test]
fn 규칙이_없어도_주소의_단지명으로_분류된다() {
    let db = db();
    category(&db, "5단지");

    let sid = add(&db, "홍길동", "○○로 123(가온마을5단지)");
    assert_eq!(cat_of(&db, sid), (Some("5단지".into()), "AUTO".into()));
}

#[test]
fn 분류하지_못하면_확인_필요가_생긴다() {
    let db = db();
    let sid = add(&db, "홍길동", "○○로 999, 1동 101호");

    assert_eq!(cat_of(&db, sid), (None, "NONE".into()));
    assert!(has_address_issue(&db, sid));
}

#[test]
fn 주소가_아예_없으면_주소_확인_필요는_만들지_않는다() {
    let db = db();
    let sid = add(&db, "홍길동", "");
    // 주소 없음은 '필수 정보 누락' 이 알린다. 주소 분류 확인까지 겹쳐 띄우지 않는다.
    assert!(!has_address_issue(&db, sid));
}

#[test]
fn 도로명을_뽑아_저장해_둔다() {
    let db = db();
    let sid = add(&db, "홍길동", "○○시 ○○로 123, 101동 1001호");
    let road: Option<String> = db
        .read(|c| {
            Ok(c.query_row(
                "SELECT address_road FROM students WHERE id = ?1",
                [sid],
                |r| r.get(0),
            )?)
        })
        .unwrap();
    assert_eq!(road.as_deref(), Some("○○로 123"));
}

// ---------------------------------------------------------------
// 직접 지정 (MANUAL)
// ---------------------------------------------------------------

#[test]
fn 이번_학생만_직접_지정할_수_있다() {
    let db = db();
    let cat = category(&db, "5단지");
    let sid = add(&db, "홍길동", "○○로 999");
    assert!(has_address_issue(&db, sid));

    db.write(|c| {
        set_manual(c, sid, Some(cat))?;
        student::sync_issues(c, sid, 2026, today())
    })
    .unwrap();

    assert_eq!(cat_of(&db, sid), (Some("5단지".into()), "MANUAL".into()));
    assert!(!has_address_issue(&db, sid), "직접 지정하면 확인 필요가 닫힌다");
}

#[test]
fn 직접_지정한_학생은_전체_재적용에서_바뀌지_않는다() {
    let db = db();
    let five = category(&db, "5단지");
    let three = category(&db, "3단지");

    let sid = add(&db, "홍길동", "○○로 123");
    db.write(|c| set_manual(c, sid, Some(five))).unwrap();

    // 같은 주소를 3단지로 보내는 규칙을 만들고 전체 재적용
    add_rule(&db, "ROAD", "○○로 123", three);
    let out = db.write(|c| reapply(c, 2026, today(), |_, _| {})).unwrap();

    assert_eq!(
        cat_of(&db, sid),
        (Some("5단지".into()), "MANUAL".into()),
        "직접 지정을 자동화가 덮으면 안 된다"
    );
    assert_eq!(out.kept_manual, 1);
    assert_eq!(out.by_rule, 0);
}

#[test]
fn 직접_지정을_풀면_다시_자동_판정한다() {
    let db = db();
    let five = category(&db, "5단지");
    let three = category(&db, "3단지");
    add_rule(&db, "ROAD", "○○로 123", three);

    let sid = add(&db, "홍길동", "○○로 123");
    db.write(|c| set_manual(c, sid, Some(five))).unwrap();
    assert_eq!(cat_of(&db, sid).0.as_deref(), Some("5단지"));

    db.write(|c| set_manual(c, sid, None)).unwrap();
    assert_eq!(cat_of(&db, sid), (Some("3단지".into()), "RULE".into()));
}

// ---------------------------------------------------------------
// 규칙을 만든 뒤 다른 학생에게 적용
// ---------------------------------------------------------------

#[test]
fn 같은_주소_학생이_몇_명인지_미리_세어_본다() {
    let db = db();
    let cat = category(&db, "5단지");

    for i in 0..5 {
        add(&db, &format!("학생{i}"), &format!("○○로 123, 10{i}동 101호"));
    }
    add(&db, "다른집", "△△로 456");
    let manual = add(&db, "직접지정", "○○로 123, 999동 1호");
    db.write(|c| set_manual(c, manual, Some(cat))).unwrap();

    let m = db
        .read(|c| count_matching(c, "ROAD", "○○로 123", 2026, today()))
        .unwrap();
    assert_eq!(m.total, 6, "직접 지정 학생도 걸리기는 한다");
    assert_eq!(m.unclassified, 5, "새로 분류될 학생");
    assert_eq!(m.manual, 1, "직접 지정이라 건드리지 않을 학생");
}

#[test]
fn 규칙을_적용하면_같은_주소_학생이_한꺼번에_분류된다() {
    let db = db();
    let cat = category(&db, "5단지");
    for i in 0..10 {
        add(&db, &format!("학생{i}"), &format!("○○로 123, 10{i}동 {i}01호"));
    }
    add(&db, "다른집", "△△로 456");

    let rid = add_rule(&db, "ROAD", "○○로 123", cat);
    let changed = db.write(|c| apply_rule(c, rid, 2026, today())).unwrap();

    assert_eq!(changed, 10);
    let rules = db.read(|c| list_rules(c, 2026, today())).unwrap();
    assert_eq!(rules[0].applied, 10, "적용 학생 수가 보여야 한다");

    // 다른 주소 학생은 그대로 미분류
    let others: i64 = db
        .read(|c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM students WHERE address_category_id IS NULL",
                [],
                |r| r.get(0),
            )?)
        })
        .unwrap();
    assert_eq!(others, 1);
}

#[test]
fn 규칙을_적용해도_직접_지정_학생은_건드리지_않는다() {
    let db = db();
    let five = category(&db, "5단지");
    let three = category(&db, "3단지");

    let manual = add(&db, "직접지정", "○○로 123, 1동 1호");
    db.write(|c| set_manual(c, manual, Some(five))).unwrap();
    let auto = add(&db, "보통학생", "○○로 123, 2동 2호");

    let rid = add_rule(&db, "ROAD", "○○로 123", three);
    db.write(|c| apply_rule(c, rid, 2026, today())).unwrap();

    assert_eq!(cat_of(&db, manual).0.as_deref(), Some("5단지"));
    assert_eq!(cat_of(&db, auto).0.as_deref(), Some("3단지"));
}

// ---------------------------------------------------------------
// 충돌
// ---------------------------------------------------------------

#[test]
fn 규칙이_부딪히면_분류하지_않고_확인_필요를_띄운다() {
    let db = db();
    let three = category(&db, "3단지");
    let five = category(&db, "5단지");
    add_rule(&db, "CONTAINS", "가온마을", three);
    add_rule(&db, "CONTAINS", "○○로", five);

    let sid = add(&db, "홍길동", "○○로 123 가온마을");

    assert_eq!(cat_of(&db, sid), (None, "CONFLICT".into()));
    assert!(has_address_issue(&db, sid));

    let st = db.read(|c| status(c, sid)).unwrap();
    assert_eq!(st.conflicts.len(), 2, "무엇이 부딪혔는지 보여 준다");
    assert!(st.reason.contains("3단지") && st.reason.contains("5단지"));
}

#[test]
fn 잘_되던_학생도_규칙이_늘어_부딪히면_확인_필요가_다시_열린다() {
    let db = db();
    let three = category(&db, "3단지");
    let five = category(&db, "5단지");
    add_rule(&db, "CONTAINS", "가온마을", three);

    let sid = add(&db, "홍길동", "○○로 123 가온마을");
    assert_eq!(cat_of(&db, sid).0.as_deref(), Some("3단지"));
    assert!(!has_address_issue(&db, sid));

    // 같은 순위의 다른 규칙이 생겼다
    add_rule(&db, "CONTAINS", "○○로", five);
    db.write(|c| {
        reapply(c, 2026, today(), |_, _| {})?;
        student::sync_issues(c, sid, 2026, today())
    })
    .unwrap();

    assert_eq!(cat_of(&db, sid).1, "CONFLICT");
    assert!(has_address_issue(&db, sid), "다시 확인이 필요해진다");
}

// ---------------------------------------------------------------
// 주소를 바꿨을 때
// ---------------------------------------------------------------

fn edit_address(db: &Db, sid: i64, addr: &str, keep_manual: bool) {
    let input = student::StudentInput {
        name: "홍길동".into(),
        gender: Some("M".into()),
        birth_raw: Some("170315".into()),
        address_raw: Some(addr.into()),
        primary_phone: Some("010-1234-5678".into()),
        school_year: 2026,
        grade: 3,
        class_name: Some("가람".into()),
        class_no: Some(1),
        keep_manual_address: keep_manual,
        ..Default::default()
    };
    db.write(|c| student::update(c, sid, &input, today())).unwrap();
}

#[test]
fn 주소를_바꾸면_예전_자동_판정이_남지_않는다() {
    let db = db();
    let five = category(&db, "5단지");
    add_rule(&db, "ROAD", "○○로 123", five);

    let sid = add(&db, "홍길동", "○○로 123, 101동 1001호");
    assert_eq!(cat_of(&db, sid).0.as_deref(), Some("5단지"));

    edit_address(&db, sid, "△△로 456, 2동 202호", false);
    assert_eq!(
        cat_of(&db, sid),
        (None, "NONE".into()),
        "다른 집으로 이사했는데 5단지로 남으면 안 된다"
    );
}

#[test]
fn 주소를_바꾸면_직접_지정도_다시_따진다() {
    let db = db();
    let five = category(&db, "5단지");
    let sid = add(&db, "홍길동", "○○로 123");
    db.write(|c| set_manual(c, sid, Some(five))).unwrap();

    edit_address(&db, sid, "△△로 456", false);
    assert_eq!(
        cat_of(&db, sid),
        (None, "NONE".into()),
        "새 주소가 5단지라고 볼 근거가 없다"
    );
}

#[test]
fn 사용자가_고르면_주소를_바꿔도_직접_지정을_남긴다() {
    let db = db();
    let five = category(&db, "5단지");
    let sid = add(&db, "홍길동", "○○로 123, 101동 1001호");
    db.write(|c| set_manual(c, sid, Some(five))).unwrap();

    // 같은 단지 안에서 동만 옮긴 경우처럼, 사용자가 유지를 고를 수 있다
    edit_address(&db, sid, "○○로 123, 105동 502호", true);
    assert_eq!(cat_of(&db, sid), (Some("5단지".into()), "MANUAL".into()));

    // 정리값은 새 주소를 따른다
    let (norm, road): (Option<String>, Option<String>) = db
        .read(|c| {
            Ok(c.query_row(
                "SELECT address_norm, address_road FROM students WHERE id = ?1",
                [sid],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?)
        })
        .unwrap();
    assert!(norm.unwrap().contains("105동"));
    assert_eq!(road.as_deref(), Some("○○로 123"));
}

#[test]
fn 주소를_안_바꾸면_직접_지정이_그대로다() {
    let db = db();
    let five = category(&db, "5단지");
    let sid = add(&db, "홍길동", "○○로 123");
    db.write(|c| set_manual(c, sid, Some(five))).unwrap();

    edit_address(&db, sid, "○○로 123", false);
    assert_eq!(cat_of(&db, sid), (Some("5단지".into()), "MANUAL".into()));
}

// ---------------------------------------------------------------
// 전체 다시 적용
// ---------------------------------------------------------------

#[test]
fn 전체_재적용_집계가_실제_자료와_맞는다() {
    let db = db();
    let five = category(&db, "5단지");
    let three = category(&db, "3단지");
    let house = db
        .read(|c| {
            Ok(c.query_row(
                "SELECT id FROM address_categories WHERE name = '주택'",
                [],
                |r| r.get::<_, i64>(0),
            )?)
        })
        .unwrap();

    add_rule(&db, "ROAD", "○○로 123", five);
    add_rule(&db, "CONTAINS", "가온빌라", house);

    // 규칙으로 분류될 학생 4명
    for i in 0..4 {
        add(&db, &format!("규칙{i}"), &format!("○○로 123, {i}01동 1호"));
    }
    // 주소의 단지명으로 자동 분류될 학생 3명
    for i in 0..3 {
        add(&db, &format!("자동{i}"), "△△로 9(나온마을3단지)");
    }
    // 포함 규칙 2명
    for i in 0..2 {
        add(&db, &format!("빌라{i}"), "△△로 77 가온빌라 201호");
    }
    // 미분류 5명
    for i in 0..5 {
        add(&db, &format!("미분류{i}"), &format!("××로 {i}, 1동 1호"));
    }
    // 주소 없음 2명
    for i in 0..2 {
        add(&db, &format!("주소없음{i}"), "");
    }
    // 직접 지정 1명
    let manual = add(&db, "직접", "○○로 123, 999동 9호");
    db.write(|c| set_manual(c, manual, Some(three))).unwrap();

    let out = db.write(|c| reapply(c, 2026, today(), |_, _| {})).unwrap();

    assert_eq!(out.total, 17);
    assert_eq!(out.by_rule, 6, "도로명 4 + 포함 2");
    assert_eq!(out.by_auto, 3);
    assert_eq!(out.unclassified, 5);
    assert_eq!(out.no_address, 2);
    assert_eq!(out.kept_manual, 1);
    assert_eq!(out.conflict, 0);

    // 집계와 실제 자료가 같아야 한다
    let real: i64 = db
        .read(|c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM students WHERE address_category_id IS NOT NULL",
                [],
                |r| r.get(0),
            )?)
        })
        .unwrap();
    assert_eq!(real, out.by_rule + out.by_auto + out.kept_manual);
}

#[test]
fn 재적용이_진행_상황을_알려_준다() {
    let db = db();
    for i in 0..5 {
        add(&db, &format!("학생{i}"), "○○로 123");
    }
    let mut seen: Vec<(usize, usize)> = Vec::new();
    db.write(|c| reapply(c, 2026, today(), |d, t| seen.push((d, t)))).unwrap();

    assert!(!seen.is_empty());
    assert_eq!(seen.last(), Some(&(5, 5)));
    assert!(seen.iter().all(|(d, t)| d <= t));
}

#[test]
fn 규칙을_지우면_그_규칙으로_분류됐던_학생이_미분류로_돌아간다() {
    let db = db();
    let cat = category(&db, "5단지");
    let rid = add_rule(&db, "ROAD", "○○로 123", cat);
    let sid = add(&db, "홍길동", "○○로 123");
    assert_eq!(cat_of(&db, sid).0.as_deref(), Some("5단지"));

    db.write(|c| delete_rule(c, rid)).unwrap();
    assert_eq!(cat_of(&db, sid), (None, "NONE".into()));
}

#[test]
fn 규칙을_꺼_두면_적용되지_않는다() {
    let db = db();
    let cat = category(&db, "5단지");
    let rid = add_rule(&db, "ROAD", "○○로 123", cat);
    let sid = add(&db, "홍길동", "○○로 123");
    assert_eq!(cat_of(&db, sid).0.as_deref(), Some("5단지"));

    db.write(|c| update_rule(c, rid, "ROAD", "○○로 123", cat, false))
        .unwrap();
    db.write(|c| reapply(c, 2026, today(), |_, _| {})).unwrap();
    assert_eq!(cat_of(&db, sid), (None, "NONE".into()));
}

// ---------------------------------------------------------------
// 판정 상태 보여 주기
// ---------------------------------------------------------------

#[test]
fn 판정_근거를_알_수_있다() {
    let db = db();
    let cat = category(&db, "5단지");
    add_rule(&db, "ROAD", "○○로 123", cat);
    let sid = add(&db, "홍길동", "○○로 123, 101동 1001호");

    let st = db.read(|c| status(c, sid)).unwrap();
    assert_eq!(st.category_name.as_deref(), Some("5단지"));
    assert_eq!(st.source, "RULE");
    assert_eq!(st.source_label, "주소 규칙으로 분류");
    assert!(st.rule_text.as_deref().unwrap().contains("○○로 123"));
    assert!(st.reason.contains("○○로 123"));
}

#[test]
fn 직접_지정_학생의_근거에_자동화가_건드리지_않는다고_적힌다() {
    let db = db();
    let cat = category(&db, "5단지");
    let sid = add(&db, "홍길동", "○○로 123");
    db.write(|c| set_manual(c, sid, Some(cat))).unwrap();

    let st = db.read(|c| status(c, sid)).unwrap();
    assert_eq!(st.source_label, "직접 지정");
    assert!(st.reason.contains("바꾸지 않습니다"), "{}", st.reason);
}

#[test]
fn 미분류_학생에게_규칙_후보를_내놓는다() {
    let db = db();
    let sid = add(&db, "홍길동", "○○시 ○○로 123, 101동 1001호(가온마을5단지)");

    let st = db.read(|c| status(c, sid)).unwrap();
    assert_eq!(st.source, "NONE");
    assert!(!st.suggestions.is_empty());
    assert_eq!(st.suggestions[0].kind, "ROAD");
    assert_eq!(st.suggestions[0].pattern, "○○로 123");
    // 동·호수는 규칙 후보에 들어가지 않는다
    assert!(st.suggestions.iter().all(|x| !x.pattern.contains('동')));
}
