//! 졸업생 저장소 검사.
//!
//! 여기서 보는 것 둘: **졸업생이 현재 명단에서 빠지는가**, 그리고
//! **졸업 당시 자리가 그대로 보이는가.** 이름은 모두 가상이다.

use super::*;
use crate::db::Db;
use crate::repo::{settings, stats, student, transition};
use chrono::NaiveDate;

fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2027, 2, 20).unwrap()
}

fn db() -> Db {
    let db = Db::memory();
    db.write(|c| {
        settings::create_year(c, 2026)?;
        settings::create_year(c, 2027)?;
        settings::set_current_year(c, 2026)
    })
    .unwrap();
    db
}

fn add(db: &Db, name: &str, grade: i32, class_name: &str, no: i32, gender: &str) -> i64 {
    let input = student::StudentInput {
        name: name.into(),
        gender: Some(gender.into()),
        birth_raw: Some("150315".into()),
        school_year: 2026,
        grade,
        class_name: Some(class_name.into()),
        class_no: Some(no),
        ..Default::default()
    };
    db.write(|c| student::create(c, &input, today())).unwrap()
}

fn graduate(db: &Db, id: i64, grade: i32, class_name: &str, no: i32) {
    db.write(|c| transition::graduate(c, id, 2026, grade, Some(class_name), Some(no), today()))
        .unwrap();
}

fn names(rows: &[GradRow]) -> Vec<String> {
    rows.iter().map(|r| r.name.clone()).collect()
}

// ---------------------------------------------------------------
// 목록
// ---------------------------------------------------------------

#[test]
fn 졸업_학년도별로_모아_센다() {
    let db = db();
    let a = add(&db, "졸업가", 6, "가람", 1, "M");
    let b = add(&db, "졸업나", 6, "가람", 2, "F");
    graduate(&db, a, 6, "가람", 1);
    graduate(&db, b, 6, "가람", 2);

    let years = db.read(years).unwrap();
    assert_eq!(years.len(), 1);
    assert_eq!(years[0].school_year, 2026);
    assert_eq!(years[0].count, 2);
}

#[test]
fn 졸업_당시_학년과_반과_번호를_보여_준다() {
    let db = db();
    let id = add(&db, "졸업가", 6, "나리", 12, "F");
    graduate(&db, id, 6, "나리", 12);

    let rows = db.read(|c| list(c, 2026, None)).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].grade, Some(6));
    assert_eq!(rows[0].class_label.as_deref(), Some("6-나리"));
    assert_eq!(rows[0].class_no, Some(12));
    assert_eq!(rows[0].gender.as_deref(), Some("F"));
    assert_eq!(rows[0].graduated_at.as_deref(), Some("2027-02-20"));
}

#[test]
fn 졸업생도_명단과_같은_차례로_나온다() {
    let db = db();
    let a = add(&db, "나중학생", 6, "나리", 1, "M");
    let b = add(&db, "먼저학생", 6, "가람", 1, "M");
    let c = add(&db, "가람둘", 6, "가람", 2, "M");
    graduate(&db, a, 6, "나리", 1);
    graduate(&db, b, 6, "가람", 1);
    graduate(&db, c, 6, "가람", 2);

    let rows = db.read(|c| list(c, 2026, None)).unwrap();
    assert_eq!(names(&rows), vec!["먼저학생", "가람둘", "나중학생"]);
}

#[test]
fn 이름으로_졸업생을_찾는다() {
    let db = db();
    let a = add(&db, "가온해", 6, "가람", 1, "M");
    let b = add(&db, "나린별", 6, "가람", 2, "F");
    graduate(&db, a, 6, "가람", 1);
    graduate(&db, b, 6, "가람", 2);

    let rows = db.read(|c| list(c, 2026, Some("나린"))).unwrap();
    assert_eq!(names(&rows), vec!["나린별"]);
}

// ---------------------------------------------------------------
// 현재 명단에서 빠진다
// ---------------------------------------------------------------

#[test]
fn 졸업생은_다음_학년도_명단과_통계에_없다() {
    let db = db();
    let up = add(&db, "올라간학생", 5, "가람", 1, "M");
    let out = add(&db, "졸업한학생", 6, "가람", 1, "F");
    db.write(|c| transition::promote(c, up, 2027, 6, Some("가람"), Some(1)))
        .unwrap();
    graduate(&db, out, 6, "가람", 1);

    // 2027학년도에는 진급한 학생만 있다
    let f = student::ListFilter {
        school_year: 2027,
        ..Default::default()
    };
    let list27 = db.read(|c| student::list(c, &f, 100, 0)).unwrap();
    assert_eq!(list27.total, 1);
    assert_eq!(list27.rows[0].name, "올라간학생");

    let counts = db
        .read(|c| stats::totals(c, &stats::StatFilter::year(2027)))
        .unwrap();
    assert_eq!(counts.total, 1, "통계도 같은 수를 센다");

    // 2026학년도 명단에는 둘 다 그대로 있다
    let f26 = student::ListFilter {
        school_year: 2026,
        ..Default::default()
    };
    assert_eq!(db.read(|c| student::list(c, &f26, 100, 0)).unwrap().total, 2);
}

// ---------------------------------------------------------------
// 되돌리기
// ---------------------------------------------------------------

#[test]
fn 졸업을_되돌리면_그_학년도_명단으로_돌아온다() {
    let db = db();
    let id = add(&db, "되돌릴학생", 6, "가람", 3, "M");
    graduate(&db, id, 6, "가람", 3);

    let year = db.write(|c| cancel(c, id, today())).unwrap();
    assert_eq!(year, 2026);
    assert!(db.read(|c| of_student(c, id)).unwrap().is_none());
    assert!(db.read(|c| list(c, 2026, None)).unwrap().is_empty());

    // 학적은 그대로였으므로 명단에 그대로 있다
    let f = student::ListFilter {
        school_year: 2026,
        ..Default::default()
    };
    assert_eq!(db.read(|c| student::list(c, &f, 100, 0)).unwrap().total, 1);
}

#[test]
fn 졸업을_되돌려도_처리한_기록은_남는다() {
    let db = db();
    let id = add(&db, "되돌릴학생", 6, "가람", 3, "M");
    graduate(&db, id, 6, "가람", 3);
    db.write(|c| cancel(c, id, today())).unwrap();

    let kinds: Vec<String> = db
        .read(|c| {
            let mut st = c.prepare(
                "SELECT kind FROM enrollment_events WHERE student_id = ?1 ORDER BY id",
            )?;
            let out: Vec<String> = st
                .query_map([id], |r| r.get(0))?
                .collect::<rusqlite::Result<_>>()?;
            drop(st);
            Ok(out)
        })
        .unwrap();
    assert_eq!(kinds, vec!["ENROLL", "GRADUATE", "CANCEL"], "지우지 않는다");
}

#[test]
fn 졸업하지_않은_학생은_되돌릴_것이_없다() {
    let db = db();
    let id = add(&db, "재학생", 6, "가람", 1, "M");
    let err = db.write(|c| cancel(c, id, today())).unwrap_err();
    assert_eq!(err.code, "NOT_FOUND");
}

#[test]
fn 졸업생은_지우지_못한다() {
    let db = db();
    let id = add(&db, "졸업생", 6, "가람", 1, "M");
    graduate(&db, id, 6, "가람", 1);

    let err = db.write(|c| student::delete(c, id)).unwrap_err();
    assert_eq!(err.code, "IN_USE", "지난 자료를 실수로 지우지 않는다");
}
