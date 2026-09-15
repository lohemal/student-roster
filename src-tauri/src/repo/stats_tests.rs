//! 인원 집계 검사.
//!
//! 여기서 보는 것 하나: **남 + 여 + 미입력 = 합계**. 이 셋이 어긋나면 사용자는
//! 어느 숫자를 믿어야 할지 알 수 없다. 이름은 모두 가상이다.

use super::*;
use crate::db::Db;
use crate::repo::{settings, student};
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

/// `gender` 는 "M" / "F" / "" (미입력)
fn add(db: &Db, name: &str, grade: i32, class_name: &str, no: i32, gender: &str) -> i64 {
    let input = student::StudentInput {
        name: name.into(),
        gender: (!gender.is_empty()).then(|| gender.to_string()),
        birth_raw: Some("170315".into()),
        school_year: 2026,
        grade,
        class_name: Some(class_name.into()),
        class_no: Some(no),
        ..Default::default()
    };
    db.write(|c| student::create(c, &input, today())).unwrap()
}

fn counts(db: &Db, grade: i32) -> Vec<ClassCount> {
    db.read(|c| class_counts(c, 2026, Some(grade))).unwrap()
}

fn find<'a>(rows: &'a [ClassCount], label: &str) -> &'a ClassCount {
    rows.iter()
        .find(|r| r.class_label == label)
        .unwrap_or_else(|| panic!("{label} 반이 없다"))
}

fn leave(db: &Db, student_id: i64) {
    db.write(|c| {
        Ok(c.execute(
            "UPDATE enrollments SET status = 'TRANSFER_OUT'
              WHERE student_id = ?1 AND school_year = 2026",
            [student_id],
        )?)
    })
    .unwrap();
}

// ---------------------------------------------------------------

#[test]
fn 반별로_남녀와_미입력을_따로_센다() {
    let db = db();
    for i in 1..=12 {
        add(&db, &format!("일반가{i:02}"), 3, "1", i, "M");
    }
    for i in 13..=23 {
        add(&db, &format!("일반나{i:02}"), 3, "1", i, "F");
    }
    add(&db, "일반다24", 3, "1", 24, ""); // 성별 미입력

    for i in 1..=11 {
        add(&db, &format!("이반가{i:02}"), 3, "2", i, "M");
    }
    for i in 12..=23 {
        add(&db, &format!("이반나{i:02}"), 3, "2", i, "F");
    }

    let rows = counts(&db, 3);
    let one = find(&rows, "3-1");
    assert_eq!((one.male, one.female, one.unknown, one.total), (12, 11, 1, 24));

    let two = find(&rows, "3-2");
    assert_eq!((two.male, two.female, two.unknown, two.total), (11, 12, 0, 23));
}

#[test]
fn 남더하기_여더하기_미입력이_합계와_같다() {
    let db = db();
    add(&db, "가학생", 4, "가람", 1, "M");
    add(&db, "나학생", 4, "가람", 2, "F");
    add(&db, "다학생", 4, "가람", 3, "");
    add(&db, "라학생", 4, "가람", 4, "");

    for r in counts(&db, 4) {
        assert_eq!(
            r.male + r.female + r.unknown,
            r.total,
            "{} 반의 숫자가 서로 맞지 않는다",
            r.class_label
        );
    }
}

#[test]
fn 성별이_비어_있는_학생을_남이나_여에_넣지_않는다() {
    let db = db();
    add(&db, "가학생", 5, "가람", 1, "");
    add(&db, "나학생", 5, "가람", 2, "");

    let rows = counts(&db, 5);
    let one = find(&rows, "5-가람");
    assert_eq!((one.male, one.female), (0, 0), "어느 쪽에도 넣지 않는다");
    assert_eq!(one.unknown, 2);
    assert_eq!(one.total, 2);
}

#[test]
fn 전출한_학생은_그_즉시_빠진다() {
    let db = db();
    let a = add(&db, "가학생", 3, "나리", 1, "M");
    add(&db, "나학생", 3, "나리", 2, "M");
    add(&db, "다학생", 3, "나리", 3, "F");

    let before = counts(&db, 3);
    assert_eq!(find(&before, "3-나리").total, 3);
    assert_eq!(find(&before, "3-나리").male, 2);

    leave(&db, a);

    let after = counts(&db, 3);
    assert_eq!(find(&after, "3-나리").male, 1, "남학생이 하나 줄었다");
    assert_eq!(find(&after, "3-나리").female, 1);
    assert_eq!(find(&after, "3-나리").total, 2);
}

#[test]
fn 다른_학년은_섞이지_않는다() {
    let db = db();
    add(&db, "가학생", 3, "가람", 1, "M");
    add(&db, "나학생", 4, "가람", 1, "M");
    add(&db, "다학생", 5, "가람", 1, "F");

    let three = counts(&db, 3);
    assert_eq!(three.len(), 1);
    assert_eq!(three[0].total, 1);
    assert_eq!(three[0].grade, 3);

    // 학년을 고르지 않으면 전 학년이 나온다 — Phase 7 통계가 쓸 모양이다
    let all = db.read(|c| class_counts(c, 2026, None)).unwrap();
    assert_eq!(all.len(), 3);
    assert_eq!(all.iter().map(|r| r.total).sum::<i64>(), 3);
}

#[test]
fn 반이_숫자면_숫자_차례로_나온다() {
    let db = db();
    for (i, name) in ["1", "2", "3", "10"].iter().enumerate() {
        add(&db, &format!("학생{i}"), 6, name, 1, "M");
    }
    let labels: Vec<String> = counts(&db, 6).into_iter().map(|r| r.class_label).collect();
    assert_eq!(labels, vec!["6-1", "6-2", "6-3", "6-10"]);
}

#[test]
fn 학년_합계도_함께_준다() {
    let db = db();
    add(&db, "가학생", 3, "가람", 1, "M");
    add(&db, "나학생", 3, "가람", 2, "F");
    add(&db, "다학생", 3, "나리", 1, "M");
    add(&db, "라학생", 3, "나리", 2, "");

    let g = db.read(|c| grade_counts(c, 2026, 3)).unwrap();
    assert_eq!(g.classes.len(), 2);
    assert_eq!((g.total.male, g.total.female, g.total.unknown), (2, 1, 1));
    assert_eq!(g.total.total, 4);
    assert!(g.total.class_label.contains("합계"));
}

#[test]
fn 그_반에서_쓰이는_번호를_알려_준다() {
    let db = db();
    add(&db, "가학생", 3, "나리", 1, "M");
    add(&db, "나학생", 3, "나리", 7, "F");
    let out = add(&db, "다학생", 3, "나리", 9, "M");

    let used = db
        .read(|c| used_numbers(c, 2026, 3, Some("나리")))
        .unwrap();
    assert_eq!(used, vec![1, 7, 9]);

    // 전출하면 그 번호는 비어 있는 것으로 본다
    leave(&db, out);
    let used = db
        .read(|c| used_numbers(c, 2026, 3, Some("나리")))
        .unwrap();
    assert_eq!(used, vec![1, 7]);
}
