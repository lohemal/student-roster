//! 내보낼 학생 읽기 검사.
//!
//! 여기서 보는 것 하나: **학생명단에 보이던 학생이 그대로, 같은 차례로 파일에 들어가는가.**
//! 화면에 48명이었는데 파일이 47명이면 둘 중 어느 쪽도 믿을 수 없게 된다.
//! 이름·연락처·주소는 모두 가상이다.

use super::*;
use crate::db::Db;
use crate::repo::{address as addr_repo, settings, sibling as sib, student, transfer};
use chrono::NaiveDate;

fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 9, 15).unwrap()
}

fn db() -> Db {
    let db = Db::memory();
    db.write(|c| {
        settings::create_year(c, 2025)?;
        settings::create_year(c, 2026)?;
        settings::set_current_year(c, 2026)
    })
    .unwrap();
    db
}

fn opt(s: &str) -> Option<String> {
    (!s.is_empty()).then(|| s.to_string())
}

/// 학생 하나. 빈 문자열은 빈칸이다.
#[allow(clippy::too_many_arguments)]
fn add(
    db: &Db,
    year: i32,
    name: &str,
    grade: i32,
    class_name: &str,
    no: Option<i32>,
    address: &str,
) -> i64 {
    let input = student::StudentInput {
        name: name.into(),
        gender: Some("M".into()),
        birth_raw: Some("170315".into()),
        address_raw: opt(address),
        school_year: year,
        grade,
        class_name: opt(class_name),
        class_no: no,
        ..Default::default()
    };
    db.write(|c| student::create(c, &input, today())).unwrap()
}

/// 보호자 정보까지 갖춘 학생. 형제 판정에 쓴다.
fn add_family(db: &Db, name: &str, grade: i32, class_name: &str, father: &str, mother: &str) -> i64 {
    let input = student::StudentInput {
        name: name.into(),
        gender: Some("F".into()),
        birth_raw: Some("170315".into()),
        father_name: opt(father),
        mother_name: opt(mother),
        father_phone: opt("010-1000-0003"),
        mother_phone: opt("010-1000-0002"),
        primary_phone: opt("010-1000-0001"),
        school_year: 2026,
        grade,
        class_name: opt(class_name),
        class_no: Some(1),
        ..Default::default()
    };
    db.write(|c| student::create(c, &input, today())).unwrap()
}

fn filter() -> ListFilter {
    ListFilter {
        school_year: 2026,
        ..Default::default()
    }
}

fn get(db: &Db, f: &ListFilter) -> Vec<Row> {
    db.read(|c| rows(c, f, today())).unwrap()
}

fn names(rows: &[Row]) -> Vec<String> {
    rows.iter().map(|r| r.name.clone()).collect()
}

/// 학생명단 화면이 보는 것과 같은 조건으로 읽은 이름.
fn roster_names(db: &Db, f: &ListFilter) -> Vec<String> {
    db.read(|c| student::list(c, f, 1000, 0, today()))
        .unwrap()
        .rows
        .into_iter()
        .map(|r| r.name)
        .collect()
}

// ---------------------------------------------------------------
// 명단과 같은가
// ---------------------------------------------------------------

#[test]
fn 명단에_보이는_학생이_그대로_들어간다() {
    let db = db();
    add(&db, 2026, "가학생", 1, "가람", Some(1), "○○시 가온로 101");
    add(&db, 2026, "나학생", 3, "나리", Some(5), "");
    add(&db, 2026, "다학생", 6, "2", Some(2), "○○시 가온로 202");

    let f = filter();
    assert_eq!(names(&get(&db, &f)), roster_names(&db, &f));
    assert_eq!(get(&db, &f).len(), 3);
}

#[test]
fn 명단과_같은_차례로_읽는다() {
    let db = db();
    // 일부러 뒤섞어 넣는다
    add(&db, 2026, "삼학년둘", 3, "가람", Some(2), "");
    add(&db, 2026, "일학년", 1, "가람", Some(1), "");
    add(&db, 2026, "삼학년하나", 3, "가람", Some(1), "");
    add(&db, 2026, "삼학년나리", 3, "나리", Some(1), "");

    let f = filter();
    let got = names(&get(&db, &f));
    assert_eq!(
        got,
        vec!["일학년", "삼학년하나", "삼학년둘", "삼학년나리"],
        "학년 → 반 → 번호"
    );
    assert_eq!(got, roster_names(&db, &f), "명단과 같은 차례");
}

#[test]
fn 번호가_없는_학생도_빠지지_않는다() {
    let db = db();
    add(&db, 2026, "번호있음", 3, "가람", Some(1), "");
    add(&db, 2026, "번호없음", 3, "가람", None, "");

    let got = get(&db, &filter());
    assert_eq!(got.len(), 2);
    let last = got.iter().find(|r| r.name == "번호없음").unwrap();
    assert_eq!(last.class_no, None, "없는 번호를 지어내지 않는다");
}

#[test]
fn 전출한_학생은_기본으로_빠진다() {
    let db = db();
    add(&db, 2026, "남은학생", 3, "가람", Some(1), "");
    let left = add(&db, 2026, "나간학생", 3, "가람", Some(2), "");

    db.write(|c| {
        transfer::transfer_out(
            c,
            &transfer::TransferOutInput {
                student_id: left,
                school_year: 2026,
                date: "2026-09-12".into(),
                to_school: Some("가상초등학교".into()),
                note: None,
            },
            today(),
        )
    })
    .unwrap();

    let f = filter();
    assert_eq!(names(&get(&db, &f)), vec!["남은학생"]);
    assert_eq!(names(&get(&db, &f)), roster_names(&db, &f));
}

// ---------------------------------------------------------------
// 조건
// ---------------------------------------------------------------

#[test]
fn 학년_조건이_그대로_쓰인다() {
    let db = db();
    add(&db, 2026, "일학년", 1, "가람", Some(1), "");
    add(&db, 2026, "삼학년", 3, "가람", Some(1), "");

    let f = ListFilter {
        grade: Some(3),
        ..filter()
    };
    assert_eq!(names(&get(&db, &f)), vec!["삼학년"]);
    assert_eq!(names(&get(&db, &f)), roster_names(&db, &f));
}

#[test]
fn 주소는_원본을_그대로_읽고_분류는_따로_읽는다() {
    let db = db();
    let cat = db
        .write(|c| {
            let id = addr_repo::create_category(c, "5단지")?;
            addr_repo::create_rule(c, "CONTAINS", "가온로 101", id, None)?;
            Ok(id)
        })
        .unwrap();
    add(
        &db,
        2026,
        "분류된학생",
        3,
        "가람",
        Some(1),
        "○○시 가온로 101, 101동 1001호",
    );

    let got = get(&db, &filter());
    assert_eq!(
        got[0].address_raw.as_deref(),
        Some("○○시 가온로 101, 101동 1001호"),
        "사용자가 적은 주소 그대로"
    );
    assert_eq!(got[0].address_category.as_deref(), Some("5단지"));

    // 분류로도 뽑을 수 있다
    let f = ListFilter {
        address_category_id: Some(cat),
        ..filter()
    };
    assert_eq!(names(&get(&db, &f)), vec!["분류된학생"]);
}

#[test]
fn 미분류와_주소_없음을_따로_뽑는다() {
    let db = db();
    db.write(|c| {
        let id = addr_repo::create_category(c, "5단지")?;
        addr_repo::create_rule(c, "CONTAINS", "가온로 101", id, None)?;
        Ok(())
    })
    .unwrap();
    add(&db, 2026, "분류됨", 3, "가람", Some(1), "○○시 가온로 101");
    add(&db, 2026, "미분류", 3, "가람", Some(2), "○○시 다른길 7");
    add(&db, 2026, "주소없음", 3, "가람", Some(3), "");

    let unclassified = ListFilter {
        address_unclassified: true,
        ..filter()
    };
    assert_eq!(names(&get(&db, &unclassified)), vec!["미분류"]);

    let none = ListFilter {
        address_none: true,
        ..filter()
    };
    assert_eq!(names(&get(&db, &none)), vec!["주소없음"], "미분류와 다른 상태다");
}

#[test]
fn 지난_학년도는_그_해의_학년과_반으로_읽는다() {
    let db = db();
    let id = add(&db, 2025, "올라간학생", 2, "가람", Some(7), "");
    db.write(|c| {
        student::ensure_enrollment(c, id, 2026, 3, Some("나리"), Some(12), "MANUAL")?;
        Ok(())
    })
    .unwrap();

    let old = get(
        &db,
        &ListFilter {
            school_year: 2025,
            ..Default::default()
        },
    );
    assert_eq!(old.len(), 1);
    assert_eq!((old[0].grade, old[0].class_no), (2, Some(7)));
    assert_eq!(old[0].class_name.as_deref(), Some("가람"));

    let now = get(&db, &filter());
    assert_eq!((now[0].grade, now[0].class_no), (3, Some(12)));
    assert_eq!(now[0].class_name.as_deref(), Some("나리"));
}

// ---------------------------------------------------------------
// 보호자 · 형제
// ---------------------------------------------------------------

#[test]
fn 보호자_연락처를_있는_그대로_읽는다() {
    let db = db();
    add_family(&db, "학생가", 3, "가람", "남궁바다", "제갈하늘");

    let got = get(&db, &filter());
    assert_eq!(got[0].primary_phone.as_deref(), Some("010-1000-0001"));
    assert_eq!(got[0].mother_phone.as_deref(), Some("010-1000-0002"));
    assert_eq!(got[0].father_phone.as_deref(), Some("010-1000-0003"));
    assert_eq!(
        got[0].father_name.as_deref(),
        Some("남궁바다"),
        "읽는 곳에서 주보호자로 덮어쓰지 않는다"
    );
}

fn confirm_all(db: &Db, student_id: i64) {
    let ids: Vec<i64> = db
        .read(|c| sib::links_of(c, student_id))
        .unwrap()
        .into_iter()
        .map(|l| l.id)
        .collect();
    for id in ids {
        db.write(|c| {
            sib::confirm(c, id)?;
            Ok(())
        })
        .unwrap();
    }
}

fn scan(db: &Db) {
    db.write(|c| sib::scan(c, 2026, today(), |_, _, _| {})).unwrap();
}

#[test]
fn 형제_이름표는_확정된_형제만_보여_준다() {
    let db = db();
    let a = add_family(&db, "첫째", 5, "가람", "남궁바다", "제갈하늘");
    add_family(&db, "둘째", 1, "나리", "남궁바다", "제갈하늘");
    scan(&db);

    // 아직 후보일 뿐이다
    let got = get(&db, &filter());
    assert!(
        got.iter().all(|r| r.siblings.is_empty()),
        "사람이 확인하기 전에는 확정으로 쓰지 않는다"
    );

    confirm_all(&db, a);
    let got = get(&db, &filter());
    let first = got.iter().find(|r| r.name == "첫째").unwrap();
    assert_eq!(first.siblings, vec!["1-나리 둘째"], "지금 학년·반으로 만든다");
    let second = got.iter().find(|r| r.name == "둘째").unwrap();
    assert_eq!(second.siblings, vec!["5-가람 첫째"]);
}

#[test]
fn 전출한_형제는_이름표에서_빠진다() {
    let db = db();
    let a = add_family(&db, "첫째", 5, "가람", "남궁바다", "제갈하늘");
    let b = add_family(&db, "둘째", 1, "나리", "남궁바다", "제갈하늘");
    scan(&db);
    confirm_all(&db, a);

    db.write(|c| {
        transfer::transfer_out(
            c,
            &transfer::TransferOutInput {
                student_id: b,
                school_year: 2026,
                date: "2026-09-12".into(),
                to_school: None,
                note: None,
            },
            today(),
        )
    })
    .unwrap();

    let got = get(&db, &filter());
    assert_eq!(names(&got), vec!["첫째"]);
    assert!(
        got[0].siblings.is_empty(),
        "관계는 남기되 지금 함께 다니는 형제만 이름표로 쓴다"
    );
}

// ---------------------------------------------------------------
// 빠르기
// ---------------------------------------------------------------

#[test]
fn 천명을_읽는_데_오래_걸리지_않는다() {
    use std::time::Instant;

    let db = db();
    db.write(|c| {
        for i in 0..1000 {
            let input = student::StudentInput {
                name: format!("학생{i:04}"),
                gender: Some("M".into()),
                birth_raw: Some("170315".into()),
                address_raw: Some("○○시 가온로 101".into()),
                primary_phone: Some("010-1000-0001".into()),
                school_year: 2026,
                grade: (i % 6) as i32 + 1,
                class_name: Some(format!("{}", i % 4 + 1)),
                class_no: Some((i % 30) as i32 + 1),
                ..Default::default()
            };
            student::create(c, &input, today())?;
        }
        Ok(())
    })
    .unwrap();

    let start = Instant::now();
    let got = get(&db, &filter());
    let elapsed = start.elapsed();

    assert_eq!(got.len(), 1000);
    assert!(elapsed.as_millis() < 3000, "1,000명 읽는 데 {elapsed:?}");
    println!("1,000명 내보내기용 읽기: {elapsed:?}");
}
