//! 학년도 전환 검사.
//!
//! 여기서 보는 것 셋:
//!   1. **지난 학년도가 그대로 남는가** — 진급은 고치는 일이 아니라 만드는 일이다.
//!   2. **프로그램이 사람 대신 정하지 않는가** — 반을 지어내지 않고, 반쯤 입력된
//!      번호를 채우지 않는다. 못 하는 일은 막고 누구인지 알려 준다.
//!   3. **중간에 실패해도 반쯤 넘어간 상태가 남지 않는가.**
//!
//! 이름·연락처는 모두 가상이다.

use super::*;
use crate::db::Db;
use crate::repo::{graduation, issue, settings, sibling, stats, student, transfer};
use rusqlite::OptionalExtension;
use chrono::NaiveDate;

const FROM: i32 = 2026;
const TO: i32 = 2027;

fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2027, 2, 20).unwrap()
}

fn db() -> Db {
    let db = Db::memory();
    db.write(|c| {
        settings::create_year(c, FROM)?;
        settings::create_year(c, TO)?;
        settings::set_current_year(c, FROM)
    })
    .unwrap();
    db
}

fn add(db: &Db, name: &str, grade: i32, class_name: &str, no: i32) -> i64 {
    add_with(db, name, grade, class_name, no, "M", "", "")
}

#[allow(clippy::too_many_arguments)]
fn add_with(
    db: &Db,
    name: &str,
    grade: i32,
    class_name: &str,
    no: i32,
    gender: &str,
    father: &str,
    mother: &str,
) -> i64 {
    let opt = |v: &str| (!v.is_empty()).then(|| v.to_string());
    let input = student::StudentInput {
        name: name.into(),
        gender: Some(gender.into()),
        birth_raw: Some("170315".into()),
        father_name: opt(father),
        mother_name: opt(mother),
        school_year: FROM,
        grade,
        class_name: Some(class_name.into()),
        class_no: Some(no),
        ..Default::default()
    };
    db.write(|c| student::create(c, &input, today())).unwrap()
}

/// 배정 줄 하나. 학생 번호로 짝짓고 기존 자리는 적지 않는다.
fn line(id: i64, name: &str, grade: Option<i32>, class_name: Option<&str>, no: Option<i32>) -> AssignRow {
    AssignRow {
        excel_row: id as usize + 1,
        school_year: Some(FROM),
        student_id: Some(id),
        old_grade: None,
        old_class: None,
        old_no: None,
        name: name.into(),
        birth: None,
        new_grade: grade,
        new_class: class_name.map(str::to_string),
        new_no: no,
    }
}

fn set(rows: Vec<AssignRow>) -> AssignSet {
    AssignSet {
        id: "test".into(),
        file_name: "진급배정.xlsx".into(),
        sheet_name: "진급배정".into(),
        from_year: FROM,
        rows,
    }
}

fn plan_of(db: &Db, assigns: Option<&AssignSet>, exclude: &[i64]) -> Plan {
    db.read(|c| build(c, FROM, TO, assigns, exclude, today()))
        .unwrap()
}

fn run(db: &Db, plan: &Plan) -> Summary {
    db.write(|c| apply(c, plan, today(), |_, _, _, _| {})).unwrap()
}

fn seat_at(db: &Db, id: i64, year: i32) -> Option<(i32, Option<String>, Option<i32>)> {
    db.read(|c| {
        Ok(c.query_row(
            "SELECT grade, class_name, class_no FROM enrollments
              WHERE student_id = ?1 AND school_year = ?2",
            rusqlite::params![id, year],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?)
    })
    .unwrap()
}

fn problem<'a>(plan: &'a Plan, kind: ProblemKind) -> Option<&'a Problem> {
    plan.problems.iter().find(|p| p.kind == kind)
}

// ---------------------------------------------------------------
// 진급
// ---------------------------------------------------------------

#[test]
fn 한_학년씩_올라가고_지난_학적은_그대로_남는다() {
    let db = db();
    let ids: Vec<i64> = (1..=5)
        .map(|g| add(&db, &format!("{g}학년학생"), g, "가람", g))
        .collect();

    let rows: Vec<AssignRow> = ids
        .iter()
        .zip(1..=5)
        .map(|(id, g)| line(*id, &format!("{g}학년학생"), Some(g + 1), Some("나리"), Some(g)))
        .collect();
    let assigns = set(rows);

    let plan = plan_of(&db, Some(&assigns), &[]);
    assert!(!plan.blocked, "{:?}", plan.problems);
    assert_eq!(plan.promote_count, 5);
    assert_eq!(plan.graduate_count, 0);

    let out = run(&db, &plan);
    assert_eq!(out.promoted, 5);

    for (id, g) in ids.iter().zip(1..=5) {
        assert_eq!(
            seat_at(&db, *id, FROM),
            Some((g, Some("가람".into()), Some(g))),
            "{g}학년 기록이 그대로여야 한다"
        );
        assert_eq!(
            seat_at(&db, *id, TO),
            Some((g + 1, Some("나리".into()), Some(g))),
            "{g}학년 → {}학년",
            g + 1
        );
    }
}

#[test]
fn 학생_번호는_그대로다() {
    let db = db();
    let id = add(&db, "올라갈학생", 3, "가람", 7);
    let assigns = set(vec![line(id, "올라갈학생", Some(4), Some("다솜"), Some(12))]);
    run(&db, &plan_of(&db, Some(&assigns), &[]));

    let ids: Vec<i64> = db
        .read(|c| {
            let mut st = c.prepare("SELECT DISTINCT student_id FROM enrollments")?;
            let out: Vec<i64> = st.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
            drop(st);
            Ok(out)
        })
        .unwrap();
    assert_eq!(ids, vec![id], "학생을 새로 만들지 않는다");
}

#[test]
fn 반_이름은_숫자도_글자도_그대로_들어간다() {
    let db = db();
    let a = add(&db, "숫자반학생", 3, "가람", 1);
    let b = add(&db, "이름반학생", 3, "가람", 2);
    let assigns = set(vec![
        line(a, "숫자반학생", Some(4), Some("2"), Some(1)),
        line(b, "이름반학생", Some(4), Some("라온"), Some(1)),
    ]);
    run(&db, &plan_of(&db, Some(&assigns), &[]));

    assert_eq!(seat_at(&db, a, TO).unwrap().1.as_deref(), Some("2"));
    assert_eq!(seat_at(&db, b, TO).unwrap().1.as_deref(), Some("라온"));
}

// ---------------------------------------------------------------
// 졸업
// ---------------------------------------------------------------

#[test]
fn 육학년은_졸업하고_다음_학적을_만들지_않는다() {
    let db = db();
    let id = add(&db, "졸업할학생", 6, "가람", 12);

    let plan = plan_of(&db, None, &[]);
    assert_eq!(plan.graduate_count, 1);
    assert!(!plan.blocked, "배정 자료가 없어도 졸업은 할 수 있다");

    let out = run(&db, &plan);
    assert_eq!(out.graduated, 1);
    assert!(seat_at(&db, id, TO).is_none());
    assert!(db.read(|c| graduation::of_student(c, id)).unwrap().is_some());
}

#[test]
fn 졸업에서_뺀_학생은_졸업도_진급도_하지_않는다() {
    let db = db();
    let stay = add(&db, "유예학생", 6, "가람", 1);
    let go = add(&db, "졸업학생", 6, "가람", 2);

    let plan = plan_of(&db, None, &[stay]);
    assert_eq!(plan.graduate_count, 1, "뺀 학생은 졸업하지 않는다");
    assert!(!plan.blocked, "막지는 않는다");

    let w = problem(&plan, ProblemKind::NoNextEnrollment).expect("알려 줘야 한다");
    assert!(!w.blocking);
    assert_eq!(w.students.len(), 1);
    assert_eq!(w.students[0].student_id, Some(stay));

    run(&db, &plan);
    assert!(db.read(|c| graduation::of_student(c, stay)).unwrap().is_none());
    assert!(seat_at(&db, stay, TO).is_none(), "7학년을 만들지 않는다");
    assert!(db.read(|c| graduation::of_student(c, go)).unwrap().is_some());
}

#[test]
fn 졸업에서_뺀_학생에게_새_반을_주면_그대로_만든다() {
    let db = db();
    let id = add(&db, "유예학생", 6, "가람", 1);
    // 사람이 배정 자료에 6학년을 한 번 더 적었다
    let assigns = set(vec![line(id, "유예학생", Some(6), Some("나리"), Some(3))]);

    let plan = plan_of(&db, Some(&assigns), &[id]);
    assert_eq!(plan.promote_count, 1);
    assert_eq!(plan.graduate_count, 0);
    assert!(
        problem(&plan, ProblemKind::UnusualGrade).is_some(),
        "기본 진급과 다르므로 알려 준다"
    );

    run(&db, &plan);
    assert_eq!(seat_at(&db, id, TO), Some((6, Some("나리".into()), Some(3))));
}

// ---------------------------------------------------------------
// 번호
// ---------------------------------------------------------------

#[test]
fn 번호가_모두_비면_이름_가나다순으로_매긴다() {
    let db = db();
    let a = add(&db, "한가람", 3, "가람", 1);
    let b = add(&db, "가온해", 3, "가람", 2);
    let c = add(&db, "나린별", 3, "가람", 3);
    let assigns = set(vec![
        line(a, "한가람", Some(4), Some("나리"), None),
        line(b, "가온해", Some(4), Some("나리"), None),
        line(c, "나린별", Some(4), Some("나리"), None),
    ]);

    let plan = plan_of(&db, Some(&assigns), &[]);
    assert!(!plan.blocked, "{:?}", plan.problems);
    run(&db, &plan);

    assert_eq!(seat_at(&db, b, TO).unwrap().2, Some(1), "가온해");
    assert_eq!(seat_at(&db, c, TO).unwrap().2, Some(2), "나린별");
    assert_eq!(seat_at(&db, a, TO).unwrap().2, Some(3), "한가람");
}

#[test]
fn 번호가_일부만_있으면_전환을_막는다() {
    let db = db();
    let a = add(&db, "번호있음", 3, "가람", 1);
    let b = add(&db, "번호없음", 3, "가람", 2);
    let assigns = set(vec![
        line(a, "번호있음", Some(4), Some("나리"), Some(1)),
        line(b, "번호없음", Some(4), Some("나리"), None),
    ]);

    let plan = plan_of(&db, Some(&assigns), &[]);
    assert!(plan.blocked);
    let p = problem(&plan, ProblemKind::PartialNumbers).unwrap();
    assert_eq!(p.students.len(), 2, "그 반 학생을 모두 보여 준다");
    assert!(p.message.contains("일부"));

    // 막힌 계획은 적용되지 않는다
    assert!(db
        .write(|c| apply(c, &plan, today(), |_, _, _, _| {}))
        .is_err());
    assert!(seat_at(&db, a, TO).is_none());
}

#[test]
fn 같은_반에_번호가_겹치면_막는다() {
    let db = db();
    let a = add(&db, "학생가", 3, "가람", 1);
    let b = add(&db, "학생나", 3, "가람", 2);
    let assigns = set(vec![
        line(a, "학생가", Some(4), Some("나리"), Some(5)),
        line(b, "학생나", Some(4), Some("나리"), Some(5)),
    ]);

    let plan = plan_of(&db, Some(&assigns), &[]);
    assert!(plan.blocked);
    let p = problem(&plan, ProblemKind::NumberDup).unwrap();
    assert_eq!(p.students.len(), 2);
}

#[test]
fn 다른_반의_같은_번호는_겹친_것이_아니다() {
    let db = db();
    let a = add(&db, "학생가", 3, "가람", 1);
    let b = add(&db, "학생나", 3, "가람", 2);
    let assigns = set(vec![
        line(a, "학생가", Some(4), Some("나리"), Some(1)),
        line(b, "학생나", Some(4), Some("다솜"), Some(1)),
    ]);
    assert!(!plan_of(&db, Some(&assigns), &[]).blocked);
}

// ---------------------------------------------------------------
// 배정 자료의 문제
// ---------------------------------------------------------------

#[test]
fn 새_반이_없으면_막고_누구인지_알려_준다() {
    let db = db();
    let a = add(&db, "반있는학생", 3, "가람", 1);
    let b = add(&db, "반없는학생", 3, "가람", 2);
    let assigns = set(vec![
        line(a, "반있는학생", Some(4), Some("나리"), Some(1)),
        line(b, "반없는학생", Some(4), Some("   "), None),
    ]);

    let plan = plan_of(&db, Some(&assigns), &[]);
    assert!(plan.blocked);
    let p = problem(&plan, ProblemKind::NoClass).unwrap();
    assert_eq!(p.students.len(), 1);
    assert_eq!(p.students[0].student_id, Some(b));
    assert!(
        p.message.contains("예전 반"),
        "임의로 예전 반을 쓰지 않는다고 알려 준다"
    );
}

#[test]
fn 배정_자료에_없는_학생은_막는다() {
    let db = db();
    let a = add(&db, "적힌학생", 3, "가람", 1);
    add(&db, "빠진학생", 3, "가람", 2);
    let assigns = set(vec![line(a, "적힌학생", Some(4), Some("나리"), Some(1))]);

    let plan = plan_of(&db, Some(&assigns), &[]);
    assert!(plan.blocked);
    let p = problem(&plan, ProblemKind::NoAssign).unwrap();
    assert_eq!(p.students.len(), 1);
    assert!(p.students[0].label.contains("빠진학생"));
}

#[test]
fn 배정_자료가_아예_없으면_진급할_수_없다고_알려_준다() {
    let db = db();
    add(&db, "올라갈학생", 3, "가람", 1);
    let plan = plan_of(&db, None, &[]);
    assert!(plan.blocked);
    assert!(problem(&plan, ProblemKind::NoAssign).is_some());
}

#[test]
fn 같은_학생이_두_번_나오면_막는다() {
    let db = db();
    let id = add(&db, "두번학생", 3, "가람", 1);
    let assigns = set(vec![
        line(id, "두번학생", Some(4), Some("나리"), Some(1)),
        line(id, "두번학생", Some(4), Some("다솜"), Some(2)),
    ]);

    let plan = plan_of(&db, Some(&assigns), &[]);
    assert!(plan.blocked);
    assert!(problem(&plan, ProblemKind::Duplicated).is_some());
}

#[test]
fn 이름이_다르면_다른_학생일_수_있다고_막는다() {
    let db = db();
    let id = add(&db, "저장된이름", 3, "가람", 1);
    let assigns = set(vec![line(id, "다른이름", Some(4), Some("나리"), Some(1))]);

    let plan = plan_of(&db, Some(&assigns), &[]);
    assert!(plan.blocked);
    assert!(problem(&plan, ProblemKind::NameMismatch).is_some());
}

#[test]
fn 없는_학생_번호는_막는다() {
    let db = db();
    add(&db, "있는학생", 3, "가람", 1);
    let assigns = set(vec![line(9999, "없는학생", Some(4), Some("나리"), Some(1))]);

    let plan = plan_of(&db, Some(&assigns), &[]);
    assert!(plan.blocked);
    assert!(problem(&plan, ProblemKind::UnknownStudent).is_some());
}

#[test]
fn 학년도가_다른_배정_자료는_막는다() {
    let db = db();
    let id = add(&db, "학생가", 3, "가람", 1);
    let mut row = line(id, "학생가", Some(4), Some("나리"), Some(1));
    row.school_year = Some(2025);
    let assigns = set(vec![row]);

    let plan = plan_of(&db, Some(&assigns), &[]);
    assert!(plan.blocked);
    assert!(problem(&plan, ProblemKind::WrongYear).is_some());
}

#[test]
fn 쓸_수_없는_새_학년은_막는다() {
    let db = db();
    let id = add(&db, "육학년학생", 6, "가람", 1);
    let assigns = set(vec![line(id, "육학년학생", Some(7), Some("나리"), Some(1))]);

    let plan = plan_of(&db, Some(&assigns), &[id]);
    assert!(plan.blocked);
    assert!(problem(&plan, ProblemKind::BadGrade).is_some());
}

#[test]
fn 기존_자리가_달라도_막지_않고_알린다() {
    let db = db();
    let id = add(&db, "학생가", 3, "가람", 7);
    let mut row = line(id, "학생가", Some(4), Some("나리"), Some(1));
    row.old_grade = Some(3);
    row.old_class = Some("가람".into());
    row.old_no = Some(5); // 파일을 만든 뒤 번호가 7로 바뀌었다
    let assigns = set(vec![row]);

    let plan = plan_of(&db, Some(&assigns), &[]);
    assert!(!plan.blocked, "배정 자체는 살린다");
    let p = problem(&plan, ProblemKind::StaleSeat).unwrap();
    assert!(!p.blocking);
    assert_eq!(p.students.len(), 1);
}

#[test]
fn 기존_학년과_반과_번호로도_짝짓는다() {
    let db = db();
    let id = add(&db, "학생가", 3, "가람", 7);
    let mut row = line(id, "학생가", Some(4), Some("나리"), Some(1));
    row.student_id = None; // 사람이 학생번호 열을 지웠다
    row.old_grade = Some(3);
    row.old_class = Some("가람".into());
    row.old_no = Some(7);
    let assigns = set(vec![row]);

    let plan = plan_of(&db, Some(&assigns), &[]);
    assert!(!plan.blocked, "{:?}", plan.problems);
    assert_eq!(plan.promote_count, 1);
    let _ = id;
}

// ---------------------------------------------------------------
// 전출 · 이미 있는 학년도
// ---------------------------------------------------------------

#[test]
fn 전출한_학생은_진급_대상이_아니다() {
    let db = db();
    let stay = add(&db, "남은학생", 3, "가람", 1);
    let left = add(&db, "나간학생", 3, "가람", 2);
    db.write(|c| {
        transfer::transfer_out(
            c,
            &transfer::TransferOutInput {
                student_id: left,
                school_year: FROM,
                date: "2026-09-12".into(),
                to_school: None,
                note: None,
            },
            NaiveDate::from_ymd_opt(2026, 9, 15).unwrap(),
        )
    })
    .unwrap();

    let assigns = set(vec![line(stay, "남은학생", Some(4), Some("나리"), Some(1))]);
    let plan = plan_of(&db, Some(&assigns), &[]);
    assert_eq!(plan.target_count, 1);
    assert!(!plan.blocked, "{:?}", plan.problems);

    run(&db, &plan);
    assert!(seat_at(&db, left, TO).is_none(), "나간 학생은 올라가지 않는다");
    assert!(seat_at(&db, stay, TO).is_some());
}

#[test]
fn 대상_학년도에_학적이_있으면_막는다() {
    let db = db();
    let id = add(&db, "학생가", 3, "가람", 1);
    // 사용자가 2027학년도 학생을 먼저 등록했다
    db.write(|c| {
        student::create(
            c,
            &student::StudentInput {
                name: "먼저등록".into(),
                school_year: TO,
                grade: 1,
                class_name: Some("가람".into()),
                class_no: Some(1),
                ..Default::default()
            },
            today(),
        )
    })
    .unwrap();

    let assigns = set(vec![line(id, "학생가", Some(4), Some("나리"), Some(1))]);
    let plan = plan_of(&db, Some(&assigns), &[]);
    assert!(plan.blocked);
    let p = problem(&plan, ProblemKind::TargetNotEmpty).unwrap();
    assert!(p.message.contains("1건"), "몇 건인지 알려 준다");
    assert_eq!(plan.existing_count, 1);
}

#[test]
fn 같은_전환을_두_번_하지_않는다() {
    let db = db();
    let id = add(&db, "학생가", 3, "가람", 1);
    let assigns = set(vec![line(id, "학생가", Some(4), Some("나리"), Some(1))]);
    run(&db, &plan_of(&db, Some(&assigns), &[]));

    let again = plan_of(&db, Some(&assigns), &[]);
    assert!(again.blocked);
    assert!(problem(&again, ProblemKind::AlreadyDone).is_some());
    assert!(problem(&again, ProblemKind::TargetNotEmpty).is_some());
}

// ---------------------------------------------------------------
// 오래된 미리보기 · 트랜잭션
// ---------------------------------------------------------------

#[test]
fn 명단이_바뀌면_미리보기의_상태_열쇠가_달라진다() {
    let db = db();
    let id = add(&db, "학생가", 3, "가람", 1);
    let assigns = set(vec![line(id, "학생가", Some(4), Some("나리"), Some(1))]);
    let before = plan_of(&db, Some(&assigns), &[]).state_key;

    add(&db, "새로온학생", 3, "가람", 2);
    let after = plan_of(&db, Some(&assigns), &[]).state_key;
    assert_ne!(before, after, "오래된 미리보기를 그대로 쓰면 안 된다");
}

#[test]
fn 중간에_실패하면_아무것도_남지_않는다() {
    let db = db();
    let a = add(&db, "학생가", 3, "가람", 1);
    let b = add(&db, "학생나", 6, "가람", 2);
    let assigns = set(vec![line(a, "학생가", Some(4), Some("나리"), Some(1))]);
    let plan = plan_of(&db, Some(&assigns), &[]);
    assert_eq!(plan.promote_count, 1);
    assert_eq!(plan.graduate_count, 1);

    // 한 트랜잭션 안에서 두 번 적용하면 두 번째가 UNIQUE 로 막힌다
    let out = db.write(|c| {
        apply(c, &plan, today(), |_, _, _, _| {})?;
        apply(c, &plan, today(), |_, _, _, _| {})
    });
    assert!(out.is_err());

    assert!(seat_at(&db, a, TO).is_none(), "진급이 남지 않았다");
    assert!(
        db.read(|c| graduation::of_student(c, b)).unwrap().is_none(),
        "졸업도 남지 않았다"
    );
}

// ---------------------------------------------------------------
// 미리보기 숫자
// ---------------------------------------------------------------

#[test]
fn 학년별_인원과_학급별_남녀를_센다() {
    let db = db();
    let a = add_with(&db, "학생가", 3, "가람", 1, "M", "", "");
    let b = add_with(&db, "학생나", 3, "가람", 2, "F", "", "");
    let c = add_with(&db, "학생다", 5, "가람", 1, "M", "", "");
    let d = add(&db, "졸업생", 6, "가람", 1);

    let assigns = set(vec![
        line(a, "학생가", Some(4), Some("나리"), Some(1)),
        line(b, "학생나", Some(4), Some("나리"), Some(2)),
        line(c, "학생다", Some(6), Some("가람"), Some(1)),
    ]);
    let plan = plan_of(&db, Some(&assigns), &[]);

    assert_eq!(plan.target_count, 4);
    assert_eq!(plan.promote_count, 3);
    assert_eq!(plan.graduate_count, 1);

    let labels: Vec<String> = plan.by_grade.iter().map(|g| g.label.clone()).collect();
    assert!(labels.contains(&"3학년 → 4학년".to_string()));
    assert!(labels.contains(&"5학년 → 6학년".to_string()));
    assert!(labels.contains(&"6학년 → 졸업".to_string()));

    let four = plan
        .classes
        .iter()
        .find(|c| c.class_label == "4-나리")
        .unwrap();
    assert_eq!((four.male, four.female, four.unknown, four.total), (1, 1, 0, 2));

    // 학생별 표
    assert_eq!(plan.rows.len(), 4);
    let row = plan.rows.iter().find(|r| r.student_id == a).unwrap();
    assert_eq!(row.current, "3-가람-1");
    assert_eq!(row.next, "4-나리-1");
    assert_eq!(row.fate, "PROMOTE");
    let out = plan.rows.iter().find(|r| r.student_id == d).unwrap();
    assert_eq!(out.next, "졸업");
}

// ---------------------------------------------------------------
// 전환 뒤에도 이어지는 것
// ---------------------------------------------------------------

#[test]
fn 형제_관계는_진급_뒤에도_이어지고_이름표만_바뀐다() {
    let db = db();
    let big = add_with(&db, "첫째", 5, "가람", 1, "M", "남궁바다", "제갈하늘");
    let small = add_with(&db, "둘째", 3, "나리", 2, "F", "남궁바다", "제갈하늘");
    db.write(|c| sibling::scan(c, FROM, today(), |_, _, _| {})).unwrap();
    let link = db.read(|c| sibling::links_of(c, big)).unwrap()[0].id;
    db.write(|c| {
        sibling::confirm(c, link)?;
        Ok(())
    })
    .unwrap();

    assert_eq!(
        db.read(|c| sibling::labels_of(c, big, FROM, today())).unwrap(),
        vec!["3-나리 둘째"]
    );

    let assigns = set(vec![
        line(big, "첫째", Some(6), Some("다솜"), Some(3)),
        line(small, "둘째", Some(4), Some("가람"), Some(5)),
    ]);
    run(&db, &plan_of(&db, Some(&assigns), &[]));

    assert_eq!(
        db.read(|c| sibling::labels_of(c, big, TO, today())).unwrap(),
        vec!["4-가람 둘째"],
        "이름표는 그 학년도 학적으로 다시 만든다"
    );
}

#[test]
fn 졸업한_형제는_다음_학년도_본교_형제에서_빠진다() {
    let db = db();
    let big = add_with(&db, "첫째", 6, "가람", 1, "M", "남궁바다", "제갈하늘");
    let small = add_with(&db, "둘째", 3, "나리", 2, "F", "남궁바다", "제갈하늘");
    db.write(|c| sibling::scan(c, FROM, today(), |_, _, _| {})).unwrap();
    let link = db.read(|c| sibling::links_of(c, big)).unwrap()[0].id;
    db.write(|c| {
        sibling::confirm(c, link)?;
        Ok(())
    })
    .unwrap();

    let assigns = set(vec![line(small, "둘째", Some(4), Some("가람"), Some(5))]);
    run(&db, &plan_of(&db, Some(&assigns), &[]));

    assert!(
        db.read(|c| sibling::labels_of(c, small, TO, today())).unwrap().is_empty(),
        "졸업한 형은 본교 형제에서 빠진다"
    );
    // 관계 자체는 남아 있고, 왜 함께 다니지 않는지 알려 준다
    let links = db.read(|c| sibling::links_of(c, small)).unwrap();
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].status, "CONFIRMED");
    let view = db.read(|c| sibling::list_for_student(c, small, TO, today())).unwrap();
    assert_eq!(view[0].partner_note.as_deref(), Some("졸업"));
}

#[test]
fn 확인_필요는_새_학년도_기준으로_다시_따진다() {
    let db = db();
    let a = add(&db, "학생가", 3, "가람", 1);
    let b = add(&db, "학생나", 3, "가람", 2);
    // 새 반에서 번호가 겹치지는 않지만, 새 학년도 표시를 만드는지 본다
    let assigns = set(vec![
        line(a, "학생가", Some(4), Some("나리"), Some(1)),
        line(b, "학생나", Some(4), Some("나리"), Some(2)),
    ]);
    run(&db, &plan_of(&db, Some(&assigns), &[]));

    // 진급 뒤 한 학생의 번호를 겹치게 바꾸면 그 학년도에서 표시가 생긴다
    db.write(|c| {
        c.execute(
            "UPDATE enrollments SET class_no = 1 WHERE student_id = ?1 AND school_year = ?2",
            rusqlite::params![b, TO],
        )?;
        student::sync_issues(c, b, TO, today())?;
        student::sync_issues(c, a, TO, today())
    })
    .unwrap();

    let kinds: Vec<String> = db
        .read(|c| Ok(issue::list_for_student(c, b)?))
        .unwrap()
        .into_iter()
        .map(|i| i.kind)
        .collect();
    assert!(kinds.contains(&"NUMBER_DUP".to_string()), "{kinds:?}");
}

#[test]
fn 전환_뒤_새_학년도_통계가_그대로_작동한다() {
    let db = db();
    let a = add_with(&db, "학생가", 3, "가람", 1, "M", "", "");
    let b = add_with(&db, "학생나", 3, "가람", 2, "F", "", "");
    add(&db, "졸업생", 6, "가람", 1);

    let assigns = set(vec![
        line(a, "학생가", Some(4), Some("나리"), Some(1)),
        line(b, "학생나", Some(4), Some("나리"), Some(2)),
    ]);
    let out = run(&db, &plan_of(&db, Some(&assigns), &[]));

    let counts = db
        .read(|c| stats::totals(c, &stats::StatFilter::year(TO), today()))
        .unwrap();
    assert_eq!(counts.total as usize, out.promoted, "진급생만 있다");
    assert_eq!((counts.male, counts.female), (1, 1));

    let old = db
        .read(|c| stats::totals(c, &stats::StatFilter::year(FROM), today()))
        .unwrap();
    assert_eq!(old.total, 3, "지난 학년도는 그대로 3명이다");
}

// ---------------------------------------------------------------
// 배정 파일 읽기
// ---------------------------------------------------------------

/// 사람이 채운 배정 파일을 하나 만든다.
fn write_form(rows: &[Vec<&str>]) -> std::path::PathBuf {
    use rust_xlsxwriter::Workbook;

    let dir = std::env::temp_dir().join(format!(
        "roster-assign-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("진급배정.xlsx");

    let mut book = Workbook::new();
    let ws = book.add_worksheet();
    let headers: Vec<&str> = crate::domain::transition::Col::ALL
        .iter()
        .map(|c| c.header())
        .collect();
    for (i, h) in headers.iter().enumerate() {
        ws.write_string(0, i as u16, *h).unwrap();
    }
    for (r, row) in rows.iter().enumerate() {
        for (i, v) in row.iter().enumerate() {
            ws.write_string(r as u32 + 1, i as u16, *v).unwrap();
        }
    }
    book.save(&path).unwrap();
    path
}

#[test]
fn 사람이_채운_배정_파일을_읽어_그대로_짝짓는다() {
    let db = db();
    let a = add(&db, "가온해", 3, "가람", 7);
    let b = add(&db, "나린별", 5, "나리", 2);

    let path = write_form(&[
        vec!["2026", &a.to_string(), "3", "가람", "7", "가온해", "17.03.15.", "4", "다솜", "12"],
        vec!["2026", &b.to_string(), "5", "나리", "2", "나린별", "17.03.15.", "6", "2", "4"],
    ]);

    let read = assign::read(path.to_str().unwrap(), None).unwrap();
    assert_eq!(read.rows.len(), 2);
    assert!(read.missing.is_empty(), "{:?}", read.missing);
    assert_eq!(read.rows[0].student_id, Some(a));
    assert_eq!(read.rows[0].new_class.as_deref(), Some("다솜"));
    assert_eq!(read.rows[0].new_no, Some(12));
    assert_eq!(read.rows[1].new_class.as_deref(), Some("2"), "숫자 반");

    let assigns = set(read.rows);
    let plan = plan_of(&db, Some(&assigns), &[]);
    assert!(!plan.blocked, "{:?}", plan.problems);
    run(&db, &plan);
    assert_eq!(seat_at(&db, a, TO), Some((4, Some("다솜".into()), Some(12))));
    assert_eq!(seat_at(&db, b, TO), Some((6, Some("2".into()), Some(4))));
}

#[test]
fn 학생번호_열이_없어도_이름과_생년월일로_찾는다() {
    let db = db();
    let id = add(&db, "가온해", 3, "가람", 7);

    // 학생번호와 기존 자리를 모두 지운 파일
    let path = write_form(&[vec![
        "2026", "", "", "", "", "가온해", "17.03.15.", "4", "다솜", "1",
    ]]);
    let read = assign::read(path.to_str().unwrap(), None).unwrap();
    let assigns = set(read.rows);

    let plan = plan_of(&db, Some(&assigns), &[]);
    assert!(!plan.blocked, "{:?}", plan.problems);
    assert_eq!(plan.promote_count, 1);
    run(&db, &plan);
    assert_eq!(seat_at(&db, id, TO).unwrap().0, 4);
}

#[test]
fn 양식이_아닌_파일은_읽지_않는다() {
    let dir = std::env::temp_dir().join(format!(
        "roster-assign-bad-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("엉뚱한파일.xlsx");
    {
        use rust_xlsxwriter::Workbook;
        let mut book = Workbook::new();
        let ws = book.add_worksheet();
        ws.write_string(0, 0, "제목").unwrap();
        ws.write_string(1, 0, "값").unwrap();
        book.save(&path).unwrap();
    }

    let err = assign::read(path.to_str().unwrap(), None).unwrap_err();
    assert_eq!(err.code, "INVALID_INPUT");
    assert!(err.user_message.contains("새 반"));
}

// ---------------------------------------------------------------
// 빠르기
// ---------------------------------------------------------------

#[test]
fn 천명_전환도_오래_걸리지_않는다() {
    use std::time::Instant;

    let db = db();
    let mut rows = Vec::new();
    db.write(|c| {
        for i in 0..1000 {
            let grade = (i % 6) as i32 + 1;
            let name = format!("학생{i:04}");
            let input = student::StudentInput {
                name: name.clone(),
                gender: Some(if i % 2 == 0 { "M" } else { "F" }.into()),
                birth_raw: Some("170315".into()),
                school_year: FROM,
                grade,
                class_name: Some(format!("{}", i % 4 + 1)),
                class_no: Some((i % 30) as i32 + 1),
                ..Default::default()
            };
            let id = student::create(c, &input, today())?;
            if grade < 6 {
                rows.push(line(
                    id,
                    &name,
                    Some(grade + 1),
                    Some(&format!("{}", i % 4 + 1)),
                    None, // 번호는 이름 차례로 매겨진다
                ));
            }
        }
        Ok(())
    })
    .unwrap();
    let assigns = set(rows);

    let t0 = Instant::now();
    let plan = plan_of(&db, Some(&assigns), &[]);
    let preview_ms = t0.elapsed();
    assert!(!plan.blocked, "{:?}", plan.problems);

    let t1 = Instant::now();
    let out = run(&db, &plan);
    let apply_ms = t1.elapsed();

    assert_eq!(out.promoted + out.graduated, 1000);
    println!(
        "1,000명 전환 — 미리보기 {preview_ms:?} · 적용 {apply_ms:?} (진급 {} · 졸업 {})",
        out.promoted, out.graduated
    );
    assert!(preview_ms.as_millis() < 5000, "미리보기 {preview_ms:?}");
    assert!(apply_ms.as_millis() < 20000, "적용 {apply_ms:?}");
}
