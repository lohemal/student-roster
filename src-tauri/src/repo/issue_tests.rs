//! 확인 필요 표의 동작 검사 — 특히 '같은 것이 두 번 쌓이지 않는다'.

use super::*;
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
