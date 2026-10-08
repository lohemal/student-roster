//! 학년도 삭제 검사.
//!
//! 여기서 보는 것 둘: **다른 학년도가 성한가**, 그리고 **학생이 남는가.**
//! 이름은 모두 가상이다.

use super::*;
use crate::db::Db;
use crate::repo::{graduation, settings, student, transition};
use chrono::NaiveDate;

fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2027, 2, 20).unwrap()
}

/// 2026(현재) + 2027 두 학년도.
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
        gender: Some("F".into()),
        birth_raw: Some("170315".into()),
        school_year: 2026,
        grade,
        class_name: Some(class_name.into()),
        class_no: Some(no),
        ..Default::default()
    };
    db.write(|c| student::create(c, &input, today())).unwrap()
}

/// 다음 학년도 학적을 하나 만든다 (전환이 하는 일과 같다).
fn promote(db: &Db, student_id: i64, grade: i32, class_name: &str, no: i32) {
    db.write(|c| transition::promote(c, student_id, 2027, grade, Some(class_name), Some(no)))
        .unwrap()
}

fn years(db: &Db) -> Vec<i32> {
    db.read(|c| {
        let mut st = c.prepare("SELECT year FROM school_years ORDER BY year")?;
        let out = st
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<Vec<i32>>>()?;
        Ok(out)
    })
    .unwrap()
}

fn enrollments(db: &Db, year: i32) -> i64 {
    db.read(|c| {
        Ok(c.query_row(
            "SELECT COUNT(*) FROM enrollments WHERE school_year = ?1",
            [year],
            |r| r.get(0),
        )?)
    })
    .unwrap()
}

fn students(db: &Db) -> i64 {
    db.read(|c| Ok(c.query_row("SELECT COUNT(*) FROM students", [], |r| r.get(0))?))
        .unwrap()
}

fn del(db: &Db, year: i32) -> AppResult<Deleted> {
    db.write(|c| delete(c, year, today()))
}

// ---------------------------------------------------------------
// A. 미래 학년도 삭제
// ---------------------------------------------------------------

#[test]
fn 미래_학년도를_지우면_현재_학년도는_그대로다() {
    let db = db();
    let a = add(&db, "가온해솔", 3, "나리", 5);
    let b = add(&db, "나린별", 3, "나리", 6);
    promote(&db, a, 4, "가람", 1);
    promote(&db, b, 4, "가람", 2);
    assert_eq!(enrollments(&db, 2027), 2);

    let out = del(&db, 2027).unwrap();
    assert_eq!(out.enrollments, 2);

    assert_eq!(years(&db), vec![2026], "2027학년도가 사라진다");
    assert_eq!(enrollments(&db, 2027), 0);
    assert_eq!(enrollments(&db, 2026), 2, "2026학년도 학적은 그대로");
    assert_eq!(students(&db), 2, "학생 기본정보는 그대로");
}

#[test]
fn 그_학년도_사건만_지운다() {
    let db = db();
    let a = add(&db, "가온해솔", 3, "나리", 5);
    promote(&db, a, 4, "가람", 1);

    let before: Vec<(i32, String)> = db
        .read(|c| {
            let mut st = c.prepare(
                "SELECT school_year, kind FROM enrollment_events WHERE student_id = ?1 ORDER BY id",
            )?;
            let out = st
                .query_map([a], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(out)
        })
        .unwrap();
    assert!(before.iter().any(|(y, k)| *y == 2026 && k == "ENROLL"));
    assert!(before.iter().any(|(y, k)| *y == 2027 && k == "PROMOTE"));

    del(&db, 2027).unwrap();

    let after: Vec<(i32, String)> = db
        .read(|c| {
            let mut st = c.prepare(
                "SELECT school_year, kind FROM enrollment_events WHERE student_id = ?1 ORDER BY id",
            )?;
            let out = st
                .query_map([a], |r| Ok((r.get(0)?, r.get(1)?)))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(out)
        })
        .unwrap();
    assert!(
        after.iter().all(|(y, _)| *y == 2026),
        "2026학년도 이력은 남아야 한다: {after:?}"
    );
    assert!(after.iter().any(|(_, k)| k == "ENROLL"));
}

// ---------------------------------------------------------------
// B. 현재 학년도 삭제 거절
// ---------------------------------------------------------------

#[test]
fn 현재_학년도는_거절하고_아무것도_바꾸지_않는다() {
    let db = db();
    add(&db, "가온해솔", 3, "나리", 5);

    let err = del(&db, 2026).unwrap_err();
    assert_eq!(err.code, "INVALID_INPUT");
    assert!(err.user_message.contains("사용 중인 학년도"));

    assert_eq!(years(&db), vec![2026, 2027], "학년도가 그대로다");
    assert_eq!(enrollments(&db, 2026), 1);
    assert_eq!(students(&db), 1);
}

#[test]
fn 지나간_학년도도_거절한다() {
    let db = db();
    db.write(|c| settings::create_year(c, 2025)).unwrap();
    let err = del(&db, 2025).unwrap_err();
    assert_eq!(err.code, "INVALID_INPUT");
    assert!(err.user_message.contains("지나간"));
    assert!(years(&db).contains(&2025));
}

#[test]
fn 없는_학년도는_NOT_FOUND() {
    let db = db();
    assert_eq!(del(&db, 2099).unwrap_err().code, "NOT_FOUND");
}

// ---------------------------------------------------------------
// C. 다른 학년도를 함께 쓰는 학생
// ---------------------------------------------------------------

#[test]
fn 두_학년도에_학적이_있는_학생은_남는다() {
    let db = db();
    let a = add(&db, "가온해솔", 3, "나리", 5);
    promote(&db, a, 4, "가람", 1);

    del(&db, 2027).unwrap();

    assert_eq!(students(&db), 1, "학생은 그대로");
    let seat: Option<(i32, Option<String>, Option<i32>)> = db
        .read(|c| {
            Ok(c.query_row(
                "SELECT grade, class_name, class_no FROM enrollments
                  WHERE student_id = ?1 AND school_year = 2026",
                [a],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?)
        })
        .unwrap();
    assert_eq!(seat, Some((3, Some("나리".into()), Some(5))), "2026 자리 그대로");
}

#[test]
fn 그_학년도_학적만_있던_학생도_지우지_않는다() {
    // 전입 예정으로 2027학년도에만 넣은 학생 — 학적은 사라져도 사람은 남는다
    let db = db();
    let a = add(&db, "가온해솔", 3, "나리", 5);
    let only_2027 = db
        .write(|c| {
            student::create(
                c,
                &student::StudentInput {
                    name: "다래슬".into(),
                    school_year: 2027,
                    grade: 1,
                    class_name: Some("가람".into()),
                    class_no: Some(1),
                    ..Default::default()
                },
                today(),
            )
        })
        .unwrap();
    promote(&db, a, 4, "가람", 2);

    let plan = db.read(|c| impact(c, 2027)).unwrap();
    assert_eq!(plan.students, 2);
    assert_eq!(plan.students_without_other_year, 1, "다래슬만 2027 전용");

    del(&db, 2027).unwrap();
    assert_eq!(students(&db), 2, "학생 기본정보는 둘 다 남는다");
    let left: i64 = db
        .read(|c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM enrollments WHERE student_id = ?1",
                [only_2027],
                |r| r.get(0),
            )?)
        })
        .unwrap();
    assert_eq!(left, 0, "학적은 사라진다");
}

#[test]
fn 형제_관계와_주소_보호자는_건드리지_않는다() {
    let db = db();
    let a = add(&db, "가온해솔", 3, "나리", 5);
    let b = add(&db, "가온바다", 5, "다솜", 2);
    db.write(|c| {
        c.execute(
            "INSERT INTO sibling_links(student_a, student_b, status, source)
             VALUES (?1, ?2, 'CONFIRMED', 'MANUAL')",
            [a.min(b), a.max(b)],
        )?;
        Ok(())
    })
    .unwrap();
    promote(&db, a, 4, "가람", 1);
    promote(&db, b, 6, "가람", 2);

    del(&db, 2027).unwrap();

    let links: i64 = db
        .read(|c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM sibling_links WHERE status = 'CONFIRMED'",
                [],
                |r| r.get(0),
            )?)
        })
        .unwrap();
    assert_eq!(links, 1, "형제 관계는 학년도가 없다");
}

// ---------------------------------------------------------------
// D. 삭제 후 다시 전환
// ---------------------------------------------------------------

#[test]
fn 전환_기록을_함께_지워_다시_전환할_수_있다() {
    let db = db();
    let a = add(&db, "가온해솔", 3, "나리", 5);
    promote(&db, a, 4, "가람", 1);
    db.write(|c| transition::record(c, 2026, 2027, "{}").map(|_| ()))
        .unwrap();
    assert!(
        db.read(|c| transition::done_before(c, 2026, 2027))
            .unwrap()
            .is_some(),
        "전환 기록이 남아 있다"
    );

    let out = del(&db, 2027).unwrap();
    assert_eq!(out.transitions, 1);
    assert!(
        db.read(|c| transition::done_before(c, 2026, 2027))
            .unwrap()
            .is_none(),
        "'이미 실행한 전환' 으로 막히면 안 된다"
    );

    // 다시 전환할 수 있다
    db.write(|c| settings::create_year(c, 2027)).unwrap();
    promote(&db, a, 4, "다솜", 3);
    assert_eq!(enrollments(&db, 2027), 1);
}

// ---------------------------------------------------------------
// 졸업 — 다른 학년도 것은 지우지 않는다
// ---------------------------------------------------------------

#[test]
fn 졸업_기록이_달린_학년도는_거절한다() {
    let db = db();
    let a = add(&db, "가온해솔", 6, "나리", 1);
    db.write(|c| transition::graduate(c, a, 2027, 6, Some("나리"), Some(1), today()))
        .unwrap();

    let plan = db.read(|c| impact(c, 2027)).unwrap();
    assert!(!plan.deletable);
    assert_eq!(plan.refusal_code.as_deref(), Some("GRADUATIONS"));
    assert_eq!(del(&db, 2027).unwrap_err().code, "INVALID_INPUT");
}

#[test]
fn 앞_학년도_졸업생은_2027을_지워도_남는다() {
    // 2026→2027 전환이 만든 졸업은 **2026학년도** 것이다
    let db = db();
    let six = add(&db, "가온해솔", 6, "나리", 1);
    let five = add(&db, "나린별", 5, "나리", 2);
    db.write(|c| transition::graduate(c, six, 2026, 6, Some("나리"), Some(1), today()))
        .unwrap();
    promote(&db, five, 6, "가람", 1);
    db.write(|c| transition::record(c, 2026, 2027, "{}").map(|_| ()))
        .unwrap();

    let plan = db.read(|c| impact(c, 2027)).unwrap();
    assert!(plan.deletable, "2027학년도에는 졸업 기록이 없다");
    assert_eq!(plan.graduations_from_transition, 1, "함께 생긴 졸업을 알린다");
    assert_eq!(plan.graduation_year, Some(2026));

    del(&db, 2027).unwrap();

    let grad = db.read(|c| graduation::of_student(c, six)).unwrap();
    assert!(grad.is_some(), "2026학년도 졸업 기록은 그대로");
    assert_eq!(grad.unwrap().school_year, 2026);
}

// ---------------------------------------------------------------
// E. 실패하면 하나도 들어가지 않는다
// ---------------------------------------------------------------

#[test]
fn 중간에_실패하면_삭제_전_상태_그대로다() {
    let db = db();
    let a = add(&db, "가온해솔", 3, "나리", 5);
    promote(&db, a, 4, "가람", 1);

    // 삭제까지 마친 뒤 일부러 실패시킨다 — 트랜잭션이 통째로 되돌아가야 한다
    let err = db
        .write(|c| {
            delete(c, 2027, today())?;
            Err::<(), _>(AppError::invalid("일부러 실패"))
        })
        .unwrap_err();
    assert_eq!(err.user_message, "일부러 실패");

    assert_eq!(years(&db), vec![2026, 2027], "학년도가 되살아난다");
    assert_eq!(enrollments(&db, 2027), 1, "학적도 그대로");
    assert_eq!(enrollments(&db, 2026), 1);
    assert_eq!(students(&db), 1);
}

// ---------------------------------------------------------------
// 영향 범위 미리보기
// ---------------------------------------------------------------

#[test]
fn 미리보기는_아무것도_바꾸지_않는다() {
    let db = db();
    let a = add(&db, "가온해솔", 3, "나리", 5);
    promote(&db, a, 4, "가람", 1);

    let plan = db.read(|c| impact(c, 2027)).unwrap();
    assert!(plan.deletable);
    assert_eq!(plan.enrollments, 1);
    assert_eq!(plan.other_enrollments, 1, "2026학년도 학적은 남는다");
    assert_eq!(plan.students, 1);

    assert_eq!(enrollments(&db, 2027), 1, "세어 보기만 했다");
    assert_eq!(years(&db), vec![2026, 2027]);
}

#[test]
fn 뒤에_학적이_남은_학년도가_있으면_막는다() {
    let db = db();
    db.write(|c| settings::create_year(c, 2028)).unwrap();
    let a = add(&db, "가온해솔", 3, "나리", 5);
    promote(&db, a, 4, "가람", 1);
    db.write(|c| transition::promote(c, a, 2028, 5, Some("가람"), Some(1)))
        .unwrap();

    let plan = db.read(|c| impact(c, 2027)).unwrap();
    assert!(!plan.deletable);
    assert_eq!(plan.refusal_code.as_deref(), Some("LATER_YEAR"));
    assert!(plan.refusal_message.unwrap().contains("2028학년도"));

    // 뒤부터 지우면 된다
    del(&db, 2028).unwrap();
    assert!(db.read(|c| impact(c, 2027)).unwrap().deletable);
}

#[test]
fn 번호_재정렬과_가져오기_기록도_함께_지운다() {
    let db = db();
    let a = add(&db, "가온해솔", 3, "나리", 5);
    promote(&db, a, 4, "가람", 1);
    db.write(|c| {
        c.execute(
            "INSERT INTO renumber_ops(school_year, grade, class_name, to_no, kind)
             VALUES (2027, 4, '가람', 3, 'ASSIGN')",
            [],
        )?;
        c.execute(
            "INSERT INTO imports(file_name, school_year) VALUES ('가상명단.xlsx', 2027)",
            [],
        )?;
        Ok(())
    })
    .unwrap();

    let plan = db.read(|c| impact(c, 2027)).unwrap();
    assert_eq!(plan.renumber_ops, 1);
    assert_eq!(plan.imports, 1);

    let out = del(&db, 2027).unwrap();
    assert_eq!(out.renumber_ops, 1);
    assert_eq!(out.imports, 1);
}

#[test]
fn 확인_필요는_남은_학년도_기준으로_다시_따진다() {
    let db = db();
    let a = add(&db, "가온해솔", 3, "나리", 5);
    // 2027학년도에는 반을 정하지 않는다 — '반·번호 미정' 이 열린다
    db.write(|c| transition::promote(c, a, 2027, 4, None, None))
        .unwrap();
    db.write(|c| student::sync_issues(c, a, 2027, today()))
        .unwrap();
    let open: i64 = db
        .read(|c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM issues WHERE student_id = ?1 AND kind = 'CLASS_ASSIGN' AND status = 'OPEN'",
                [a],
                |r| r.get(0),
            )?)
        })
        .unwrap();
    assert_eq!(open, 1);

    let out = del(&db, 2027).unwrap();
    assert_eq!(out.resynced, 1);

    let still: i64 = db
        .read(|c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM issues WHERE student_id = ?1 AND kind = 'CLASS_ASSIGN' AND status = 'OPEN'",
                [a],
                |r| r.get(0),
            )?)
        })
        .unwrap();
    assert_eq!(still, 0, "2026학년도에는 반과 번호가 있다");
}
