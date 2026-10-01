//! 학년도 전환 저장소 검사.
//!
//! 여기서 보는 것 하나: **지난 학년도가 그대로 남는가.**
//! 진급은 고치는 일이 아니라 새로 만드는 일이다. 이름은 모두 가상이다.

use super::*;
use crate::db::Db;
use crate::repo::{graduation, settings, student, transfer};
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

fn add(db: &Db, name: &str, grade: i32, class_name: &str, no: i32) -> i64 {
    let input = student::StudentInput {
        name: name.into(),
        gender: Some("M".into()),
        birth_raw: Some("170315".into()),
        school_year: 2026,
        grade,
        class_name: Some(class_name.into()),
        class_no: Some(no),
        ..Default::default()
    };
    db.write(|c| student::create(c, &input, today())).unwrap()
}

/// 그 학년도 학적을 그대로 읽는다.
fn enrollment(db: &Db, student_id: i64, year: i32) -> Option<(i32, Option<String>, Option<i32>)> {
    db.read(|c| {
        Ok(c.query_row(
            "SELECT grade, class_name, class_no FROM enrollments
              WHERE student_id = ?1 AND school_year = ?2",
            rusqlite::params![student_id, year],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?)
    })
    .unwrap()
}

fn events(db: &Db, student_id: i64) -> Vec<(i32, String)> {
    db.read(|c| {
        let mut st = c.prepare(
            "SELECT school_year, kind FROM enrollment_events
              WHERE student_id = ?1 ORDER BY id",
        )?;
        let out: Vec<(i32, String)> = st
            .query_map([student_id], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        drop(st);
        Ok(out)
    })
    .unwrap()
}

// ---------------------------------------------------------------
// 대상 고르기
// ---------------------------------------------------------------

#[test]
fn 전환_대상은_그_학년도_재학생이다() {
    let db = db();
    add(&db, "남은학생", 3, "가람", 1);
    let left = add(&db, "나간학생", 3, "가람", 2);

    db.write(|c| {
        transfer::transfer_out(
            c,
            &transfer::TransferOutInput {
                student_id: left,
                school_year: 2026,
                date: "2026-09-12".into(),
                to_school: None,
                note: None,
            },
            NaiveDate::from_ymd_opt(2026, 9, 15).unwrap(),
        )
    })
    .unwrap();

    let rows = db.read(|c| seats(c, 2026, today())).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "남은학생", "전출한 학생은 진급 대상이 아니다");
}

#[test]
fn 나갔다_돌아온_학생은_전환_대상이다() {
    let db = db();
    let id = add(&db, "돌아온학생", 3, "가람", 1);
    db.write(|c| {
        transfer::transfer_out(
            c,
            &transfer::TransferOutInput {
                student_id: id,
                school_year: 2026,
                date: "2026-09-12".into(),
                to_school: None,
                note: None,
            },
            NaiveDate::from_ymd_opt(2026, 9, 15).unwrap(),
        )?;
        transfer::transfer_out_cancel(c, id, 2026, NaiveDate::from_ymd_opt(2026, 9, 16).unwrap())
    })
    .unwrap();

    let rows = db.read(|c| seats(c, 2026, today())).unwrap();
    assert_eq!(rows.len(), 1, "지금 다니고 있으면 대상이다");
}

#[test]
fn 이미_졸업한_학생은_다시_전환하지_않는다() {
    let db = db();
    let id = add(&db, "졸업한학생", 6, "가람", 1);
    db.write(|c| graduate(c, id, 2026, 6, Some("가람"), Some(1), today()))
        .unwrap();

    assert!(db.read(|c| seats(c, 2026, today())).unwrap().is_empty());
}

#[test]
fn 대상은_명단과_같은_차례로_나온다() {
    let db = db();
    add(&db, "삼학년둘", 3, "가람", 2);
    add(&db, "일학년", 1, "가람", 1);
    add(&db, "삼학년하나", 3, "가람", 1);

    let names: Vec<String> = db
        .read(|c| seats(c, 2026, today()))
        .unwrap()
        .into_iter()
        .map(|s| s.name)
        .collect();
    assert_eq!(names, vec!["일학년", "삼학년하나", "삼학년둘"]);
}

// ---------------------------------------------------------------
// 진급 — 만들기만 한다
// ---------------------------------------------------------------

#[test]
fn 진급은_지난_학적을_고치지_않고_새로_만든다() {
    let db = db();
    let id = add(&db, "올라갈학생", 3, "가람", 7);

    db.write(|c| promote(c, id, 2027, 4, Some("다솜"), Some(12)))
        .unwrap();

    assert_eq!(
        enrollment(&db, id, 2026),
        Some((3, Some("가람".into()), Some(7))),
        "2026학년도는 그대로다"
    );
    assert_eq!(
        enrollment(&db, id, 2027),
        Some((4, Some("다솜".into()), Some(12))),
        "2027학년도가 새로 생겼다"
    );
}

#[test]
fn 진급하면_학적_이동에_진급이_남는다() {
    let db = db();
    let id = add(&db, "올라갈학생", 3, "가람", 7);
    db.write(|c| promote(c, id, 2027, 4, Some("다솜"), Some(12)))
        .unwrap();

    let ev = events(&db, id);
    assert_eq!(ev.first().map(|e| e.1.as_str()), Some("ENROLL"));
    assert_eq!(ev.last(), Some(&(2027, "PROMOTE".to_string())));
}

#[test]
fn 일곱학년으로는_올리지_않는다() {
    let db = db();
    let id = add(&db, "육학년학생", 6, "가람", 1);
    let err = db
        .write(|c| promote(c, id, 2027, 7, Some("가람"), Some(1)))
        .unwrap_err();
    assert_eq!(err.code, "INVALID_INPUT");
}

#[test]
fn 같은_학년도에_학적을_두_번_만들지_않는다() {
    let db = db();
    let id = add(&db, "올라갈학생", 3, "가람", 7);
    db.write(|c| promote(c, id, 2027, 4, Some("다솜"), Some(12)))
        .unwrap();
    // UNIQUE(student_id, school_year) 가 막는다
    assert!(db
        .write(|c| promote(c, id, 2027, 4, Some("다솜"), Some(13)))
        .is_err());
}

// ---------------------------------------------------------------
// 졸업
// ---------------------------------------------------------------

#[test]
fn 졸업은_기록을_만들고_다음_학적을_만들지_않는다() {
    let db = db();
    let id = add(&db, "졸업할학생", 6, "가람", 12);

    db.write(|c| graduate(c, id, 2026, 6, Some("가람"), Some(12), today()))
        .unwrap();

    assert!(enrollment(&db, id, 2027).is_none(), "다음 학적은 없다");
    assert_eq!(
        enrollment(&db, id, 2026),
        Some((6, Some("가람".into()), Some(12))),
        "졸업 당시 자리는 그대로 남는다"
    );
    let ev = events(&db, id);
    assert_eq!(ev.last(), Some(&(2026, "GRADUATE".to_string())));

    let g = db.read(|c| graduation::of_student(c, id)).unwrap().unwrap();
    assert_eq!(g.school_year, 2026);
    assert_eq!(g.grade, Some(6));
    assert_eq!(g.class_no, Some(12));
}

// ---------------------------------------------------------------
// 학년도 상태 · 기록
// ---------------------------------------------------------------

#[test]
fn 대상_학년도에_학적이_있는지_센다() {
    let db = db();
    let id = add(&db, "올라갈학생", 3, "가람", 7);

    let before = db.read(|c| year_state(c, 2027, today())).unwrap();
    assert!(before.exists, "학년도는 만들어 두었다");
    assert_eq!(before.enrollments, 0);

    db.write(|c| promote(c, id, 2027, 4, Some("다솜"), Some(12)))
        .unwrap();
    let after = db.read(|c| year_state(c, 2027, today())).unwrap();
    assert_eq!(after.enrollments, 1);
    assert_eq!(after.active, 1);
}

#[test]
fn 전환_기록에는_인원만_남는다() {
    let db = db();
    db.write(|c| record(c, 2026, 2027, r#"{"promoted":872,"graduated":183}"#))
        .unwrap();

    assert!(db.read(|c| done_before(c, 2026, 2027)).unwrap().is_some());
    assert!(
        db.read(|c| done_before(c, 2027, 2028)).unwrap().is_none(),
        "다른 전환은 따로 본다"
    );

    let rows = db.read(|c| history(c, 10)).unwrap();
    assert_eq!(rows.len(), 1);
    let summary = rows[0].summary.clone().unwrap();
    assert!(summary.contains("872"));
    assert!(
        !summary.contains("학생"),
        "개인정보는 넣지 않는다: {summary}"
    );
}

// ---------------------------------------------------------------
// 원본이 그대로인가
// ---------------------------------------------------------------

#[test]
fn 명단이_바뀌면_상태_열쇠가_달라진다() {
    let db = db();
    let id = add(&db, "학생하나", 3, "가람", 7);
    let key = db.read(|c| state_key(c, 2026, today())).unwrap();

    // 같은 자료를 다시 읽으면 같은 값
    assert_eq!(db.read(|c| state_key(c, 2026, today())).unwrap(), key);

    db.write(|c| {
        c.execute(
            "UPDATE enrollments SET class_no = 9 WHERE student_id = ?1 AND school_year = 2026",
            [id],
        )?;
        Ok(())
    })
    .unwrap();
    assert_ne!(db.read(|c| state_key(c, 2026, today())).unwrap(), key);
}
