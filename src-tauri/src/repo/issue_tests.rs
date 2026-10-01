//! 확인 필요 표의 동작 검사 — 특히 '같은 것이 두 번 쌓이지 않는다'.

use super::*;
use chrono::NaiveDate;

/// 검사는 시스템 시계를 보지 않는다 — 기준일을 직접 정한다.
fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 9, 15).unwrap()
}
use crate::db::Db;

fn db_with_student() -> (Db, i64) {
    let db = Db::memory();
    let id = db
        .write(|c| {
            c.execute("INSERT INTO school_years(year, is_current) VALUES (2026, 1)", [])?;
            c.execute("INSERT INTO students(name) VALUES ('홍길동')", [])?;
            let id = c.last_insert_rowid();
            c.execute(
                "INSERT INTO enrollments(student_id, school_year, grade) VALUES (?1, 2026, 3)",
                [id],
            )?;
            Ok(id)
        })
        .unwrap();
    (db, id)
}

#[test]
fn 같은_종류를_두_번_열면_문구만_바뀐다() {
    let (db, id) = db_with_student();

    db.write(|c| open(c, id, IssueKind::Birth, "처음 문구", None, None))
        .unwrap();
    db.write(|c| open(c, id, IssueKind::Birth, "나중 문구", None, None))
        .unwrap();

    let rows = db.read(|c| list_for_student(c, id)).unwrap();
    assert_eq!(rows.len(), 1, "두 줄이 되면 안 된다");
    assert_eq!(rows[0].message, "나중 문구");
}

#[test]
fn 닫으면_목록에서_사라진다() {
    let (db, id) = db_with_student();
    db.write(|c| open(c, id, IssueKind::Birth, "문구", None, None))
        .unwrap();
    db.write(|c| close(c, id, IssueKind::Birth)).unwrap();

    assert!(db.read(|c| list_for_student(c, id)).unwrap().is_empty());
}

#[test]
fn 닫았다_다시_열_수_있다() {
    let (db, id) = db_with_student();
    db.write(|c| open(c, id, IssueKind::Birth, "처음", None, None))
        .unwrap();
    db.write(|c| close(c, id, IssueKind::Birth)).unwrap();
    db.write(|c| open(c, id, IssueKind::Birth, "다시", None, None))
        .unwrap();

    let rows = db.read(|c| list_for_student(c, id)).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].message, "다시");
}

#[test]
fn 참조가_다르면_따로_쌓인다() {
    // 형제 후보처럼 상대가 여럿이면 상대마다 한 줄이어야 한다
    let (db, id) = db_with_student();
    db.write(|c| open(c, id, IssueKind::SiblingCandidate, "형제 후보 A", None, Some(11)))
        .unwrap();
    db.write(|c| open(c, id, IssueKind::SiblingCandidate, "형제 후보 B", None, Some(22)))
        .unwrap();

    assert_eq!(db.read(|c| list_for_student(c, id)).unwrap().len(), 2);
}

#[test]
fn 종류마다_사람이_읽을_이름이_있다() {
    for kind in [
        IssueKind::Address,
        IssueKind::Birth,
        IssueKind::GuardianConflict,
        IssueKind::GuardianFill,
        IssueKind::SiblingCandidate,
        IssueKind::Duplicate,
        IssueKind::Missing,
        IssueKind::ClassAssign,
        IssueKind::NumberDup,
    ] {
        let label = IssueKind::label(kind.code());
        assert!(!label.is_empty());
        assert_ne!(label, "기타 확인", "{} 에 이름이 없다", kind.code());
    }
}

#[test]
fn 없는_종류는_기타로_보여준다() {
    assert_eq!(IssueKind::label("무엇인가"), "기타 확인");
}

// ---------------------------------------------------------------
// 확인 필요 화면용 목록
// ---------------------------------------------------------------

/// 학생 하나를 반·번호까지 정해 넣는다. 이름은 가상이다.
fn add_student(db: &Db, name: &str, grade: i32, class_name: &str, class_no: i32) -> i64 {
    let name = name.to_string();
    let class_name = class_name.to_string();
    db.write(|c| {
        c.execute("INSERT INTO students(name) VALUES (?1)", [&name])?;
        let id = c.last_insert_rowid();
        c.execute(
            "INSERT INTO enrollments(student_id, school_year, grade, class_name, class_no)
             VALUES (?1, 2026, ?2, ?3, ?4)",
            params![id, grade, class_name, class_no],
        )?;
        Ok(id)
    })
    .unwrap()
}

fn find(db: &Db, f: IssueFilter) -> Vec<IssueListRow> {
    db.read(|c| list(c, &f, 100, 0, today())).unwrap().rows
}

fn year(school_year: i32) -> IssueFilter {
    IssueFilter {
        school_year,
        ..Default::default()
    }
}

/// 세 학생에게 서로 다른 확인 필요를 붙여 둔 자료
fn sample() -> Db {
    let db = Db::memory();
    db.write(|c| {
        Ok(c.execute(
            "INSERT INTO school_years(year, is_current) VALUES (2026, 1)",
            [],
        )?)
    })
    .unwrap();

    let a = add_student(&db, "학생가", 3, "나리", 7);
    let b = add_student(&db, "학생나", 3, "가람", 2);
    let c_id = add_student(&db, "학생다", 5, "나리", 11);

    db.write(|c| {
        open(c, a, IssueKind::Birth, "생년월일 '17.13.42.' 을 확인해 주세요.", None, None)?;
        open(c, a, IssueKind::Address, "주소 분류를 정하지 못했습니다.", None, None)?;
        open(c, b, IssueKind::NumberDup, "3-가람반에서 2번을 쓰는 학생이 2명 있습니다.", None, None)?;
        open(c, c_id, IssueKind::SiblingCandidate, "3-나리 학생가 학생과 형제인지 확인해 주세요.", None, Some(1))
    })
    .unwrap();
    db
}

#[test]
fn 기본은_현재_학년도_미해결만_보여_준다() {
    let db = sample();
    let rows = find(&db, year(2026));
    assert_eq!(rows.len(), 4);
    assert!(rows.iter().all(|r| r.status == "OPEN"));
    assert!(rows.iter().all(|r| r.status_label == "미해결"));

    // 다른 학년도에는 아무것도 없다
    assert!(find(&db, year(2027)).is_empty());
}

#[test]
fn 이름표는_지금_학적으로_만든다() {
    let db = sample();
    let rows = find(&db, year(2026));
    let birth = rows.iter().find(|r| r.kind == "BIRTH").unwrap();
    assert_eq!(birth.student_label, "3-나리 학생가");
    assert_eq!(birth.class_no, Some(7));

    // 반이 바뀌면 이름표도 따라 바뀐다
    let sid = birth.student_id;
    db.write(|c| {
        Ok(c.execute(
            "UPDATE enrollments SET class_name = '다솜' WHERE student_id = ?1",
            [sid],
        )?)
    })
    .unwrap();
    let again = find(&db, year(2026));
    let birth = again.iter().find(|r| r.kind == "BIRTH").unwrap();
    assert_eq!(birth.student_label, "3-다솜 학생가");
}

#[test]
fn 학년과_반으로_추린다() {
    let db = sample();

    let g3 = find(
        &db,
        IssueFilter {
            grade: Some(3),
            ..year(2026)
        },
    );
    assert_eq!(g3.len(), 3);

    let nari3 = find(
        &db,
        IssueFilter {
            grade: Some(3),
            class_name: Some("나리".into()),
            ..year(2026)
        },
    );
    assert_eq!(nari3.len(), 2, "3-나리 학생가의 두 건만");
    assert!(nari3.iter().all(|r| r.student_label == "3-나리 학생가"));
}

#[test]
fn 종류로_추린다() {
    let db = sample();
    let only = find(
        &db,
        IssueFilter {
            kind: Some("NUMBER_DUP".into()),
            ..year(2026)
        },
    );
    assert_eq!(only.len(), 1);
    assert_eq!(only[0].kind_label, "번호 중복");
    assert!(only[0].message.contains("2번"), "무엇을 확인할지 문장으로 보여 준다");
}

#[test]
fn 학생_이름과_이름표로_찾는다() {
    let db = sample();

    let by_name = find(
        &db,
        IssueFilter {
            q: Some("학생가".into()),
            ..year(2026)
        },
    );
    assert_eq!(by_name.len(), 2);

    // 화면에 보이는 그대로 쳐도 찾아야 한다
    let by_label = find(
        &db,
        IssueFilter {
            q: Some("3-나리 학생가".into()),
            ..year(2026)
        },
    );
    assert_eq!(by_label.len(), 2);

    // 반만 쳐도 걸린다
    let by_class = find(
        &db,
        IssueFilter {
            q: Some("3-가람".into()),
            ..year(2026)
        },
    );
    assert_eq!(by_class.len(), 1);
}

#[test]
fn 해결된_것도_따로_볼_수_있다() {
    let db = sample();
    let birth = find(&db, year(2026))
        .into_iter()
        .find(|r| r.kind == "BIRTH")
        .unwrap();
    db.write(|c| close(c, birth.student_id, IssueKind::Birth)).unwrap();

    assert_eq!(find(&db, year(2026)).len(), 3, "미해결에서 빠진다");

    let resolved = find(
        &db,
        IssueFilter {
            status: Some("RESOLVED".into()),
            ..year(2026)
        },
    );
    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0].status_label, "해결됨");
    assert!(resolved[0].resolved_at.is_some(), "언제 해결됐는지 남는다");

    assert_eq!(
        find(
            &db,
            IssueFilter {
                status: Some("ALL".into()),
                ..year(2026)
            }
        )
        .len(),
        4
    );
}

#[test]
fn 종류마다_열_곳을_알려_준다() {
    let db = sample();
    for r in find(&db, year(2026)) {
        let expected = match r.kind.as_str() {
            "SIBLING_CANDIDATE" | "GUARDIAN_FILL" | "GUARDIAN_CONFLICT" => "sibling",
            _ => "basic",
        };
        assert_eq!(r.tab, expected, "{} 는 {expected} 칸에서 고친다", r.kind);
    }
}

#[test]
fn 쪽_나누기와_전체_수가_맞는다() {
    let db = sample();
    let page = db.read(|c| list(c, &year(2026), 2, 0, today())).unwrap();
    assert_eq!(page.total, 4, "전체 수는 쪽과 상관없다");
    assert_eq!(page.rows.len(), 2);

    let next = db.read(|c| list(c, &year(2026), 2, 2, today())).unwrap();
    assert_eq!(next.rows.len(), 2);

    let first: Vec<i64> = page.rows.iter().map(|r| r.id).collect();
    assert!(
        next.rows.iter().all(|r| !first.contains(&r.id)),
        "같은 줄이 두 쪽에 나오면 안 된다"
    );
}

#[test]
fn 미해결이_해결됨보다_먼저_나온다() {
    let db = sample();
    let birth = find(&db, year(2026))
        .into_iter()
        .find(|r| r.kind == "BIRTH")
        .unwrap();
    db.write(|c| close(c, birth.student_id, IssueKind::Birth)).unwrap();

    let all = find(
        &db,
        IssueFilter {
            status: Some("ALL".into()),
            ..year(2026)
        },
    );
    let first_resolved = all.iter().position(|r| r.status != "OPEN").unwrap();
    assert!(
        all.iter().take(first_resolved).all(|r| r.status == "OPEN"),
        "해결된 것이 위로 올라오면 안 된다"
    );
}
