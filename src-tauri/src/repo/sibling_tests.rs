//! 형제 관계 저장소 검사.
//!
//! **사용자 결정을 자동화가 뒤집지 않는가**가 가장 중요하다.
//! 한 번 '형제 아님' 이라고 한 쌍이 다시 훑을 때마다 되살아나면 쓸 수 없는 프로그램이 된다.
//! 시험에 쓰는 이름·번호는 모두 가상이다.

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

/// 학생 하나를 보호자 정보와 함께 넣는다. 빈 문자열은 빈칸이다.
fn add(db: &Db, name: &str, fname: &str, mname: &str, fphone: &str, mphone: &str) -> i64 {
    add_at(db, name, 3, "가람", fname, mname, fphone, mphone)
}

#[allow(clippy::too_many_arguments)]
fn add_at(
    db: &Db,
    name: &str,
    grade: i32,
    class_name: &str,
    fname: &str,
    mname: &str,
    fphone: &str,
    mphone: &str,
) -> i64 {
    let opt = |s: &str| (!s.is_empty()).then(|| s.to_string());
    let input = student::StudentInput {
        name: name.into(),
        gender: Some("M".into()),
        birth_raw: Some("170315".into()),
        father_name: opt(fname),
        mother_name: opt(mname),
        father_phone: opt(fphone),
        mother_phone: opt(mphone),
        school_year: 2026,
        grade,
        class_name: Some(class_name.into()),
        class_no: Some(1),
        ..Default::default()
    };
    db.write(|c| student::create(c, &input, today())).unwrap()
}

fn run_scan(db: &Db) -> ScanResult {
    db.write(|c| {
        let out = scan(c, 2026, today(), |_, _, _| {})?;
        // 훑은 뒤에는 화면 표시를 다시 맞춘다 (명령이 하는 일과 같다)
        let ids: Vec<i64> = c
            .prepare("SELECT student_id FROM enrollments WHERE school_year = 2026")?
            .query_map([], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        for id in ids {
            student::sync_issues(c, id, 2026, today())?;
        }
        Ok(out)
    })
    .unwrap()
}

fn links(db: &Db, student_id: i64) -> Vec<SiblingView> {
    db.read(|c| list_for_student(c, student_id, 2026, today())).unwrap()
}

fn issues_of(db: &Db, student_id: i64, kind: &str) -> Vec<issue::IssueRow> {
    db.read(|c| Ok(issue::list_for_student(c, student_id)?))
        .unwrap()
        .into_iter()
        .filter(|i| i.kind == kind)
        .collect()
}

// ---------------------------------------------------------------
// 후보 찾기
// ---------------------------------------------------------------

#[test]
fn 두_항목이_같으면_후보가_된다() {
    let db = db();
    let a = add(&db, "첫째", "가철수", "가영희", "", "");
    let b = add(&db, "둘째", "가철수", "가영희", "", "");

    let out = run_scan(&db);
    assert_eq!(out.new_candidates, 1);

    let v = links(&db, a);
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].student_id, b);
    assert_eq!(v[0].status, "CANDIDATE");
    assert_eq!(v[0].status_label, "후보");
    assert_eq!(v[0].matched.len(), 2);
}

#[test]
fn 후보가_생기면_확인_필요가_함께_뜬다() {
    let db = db();
    let a = add(&db, "첫째", "가철수", "가영희", "", "");
    add(&db, "둘째", "가철수", "가영희", "", "");
    run_scan(&db);

    let found = issues_of(&db, a, "SIBLING_CANDIDATE");
    assert_eq!(found.len(), 1);
    assert!(found[0].message.contains("형제인지 확인"), "{}", found[0].message);
}

#[test]
fn 하나만_같으면_후보가_되지_않는다() {
    let db = db();
    add(&db, "가", "가철수", "가영희", "", "");
    add(&db, "나", "가철수", "나영희", "", "");

    let out = run_scan(&db);
    assert_eq!(out.new_candidates, 0);
}

#[test]
fn 자료가_빈_학생끼리_형제가_되지_않는다() {
    let db = db();
    for i in 0..30 {
        add(&db, &format!("빈칸{i}"), "", "", "", "");
    }
    let out = run_scan(&db);
    assert_eq!(out.new_candidates, 0, "빈칸으로 후보가 생기면 안 된다");
}

#[test]
fn 형제_셋이면_세_쌍이_생긴다() {
    let db = db();
    let a = add(&db, "첫째", "가철수", "가영희", "010-1111-1111", "");
    let b = add(&db, "둘째", "가철수", "가영희", "010-1111-1111", "");
    let c = add(&db, "셋째", "가철수", "가영희", "010-1111-1111", "");

    let out = run_scan(&db);
    assert_eq!(out.new_candidates, 3, "A-B, A-C, B-C");
    assert_eq!(links(&db, a).len(), 2);
    assert_eq!(links(&db, b).len(), 2);
    assert_eq!(links(&db, c).len(), 2);
}

#[test]
fn 전출한_학생은_훑지_않는다() {
    let db = db();
    let a = add(&db, "재학", "가철수", "가영희", "", "");
    let b = add(&db, "전출", "가철수", "가영희", "", "");
    db.write(|c| {
        c.execute(
            "UPDATE enrollments SET status = 'TRANSFER_OUT' WHERE student_id = ?1",
            [b],
        )?;
        Ok(())
    })
    .unwrap();

    let out = run_scan(&db);
    assert_eq!(out.new_candidates, 0);
    assert!(links(&db, a).is_empty());
}

#[test]
fn 졸업생도_훑지_않는다() {
    let db = db();
    add(&db, "재학", "가철수", "가영희", "", "");
    let b = add(&db, "졸업", "가철수", "가영희", "", "");
    db.write(|c| {
        c.execute(
            "INSERT INTO graduations(student_id, school_year) VALUES (?1, 2026)",
            [b],
        )?;
        Ok(())
    })
    .unwrap();

    assert_eq!(run_scan(&db).new_candidates, 0);
}

// ---------------------------------------------------------------
// 사용자 결정을 지킨다
// ---------------------------------------------------------------

#[test]
fn 형제_아님이라고_한_쌍은_다시_훑어도_되살아나지_않는다() {
    let db = db();
    let a = add(&db, "첫째", "가철수", "가영희", "", "");
    add(&db, "둘째", "가철수", "가영희", "", "");
    run_scan(&db);

    let link_id = links(&db, a)[0].link_id;
    db.write(|c| {
        reject(c, link_id)?;
        student::sync_issues(c, a, 2026, today())
    })
    .unwrap();

    // 여러 번 다시 훑어도 그대로여야 한다
    for _ in 0..3 {
        let out = run_scan(&db);
        assert_eq!(out.new_candidates, 0, "다시 후보로 만들면 안 된다");
        assert_eq!(out.rejected, 1);
    }

    let v = links(&db, a);
    assert_eq!(v.len(), 1);
    assert_eq!(v[0].status, "REJECTED");
    assert_eq!(v[0].status_label, "형제 아님");
    assert!(
        issues_of(&db, a, "SIBLING_CANDIDATE").is_empty(),
        "다시 묻지 않는다"
    );
}

#[test]
fn 형제로_확인한_관계도_다시_훑어도_그대로다() {
    let db = db();
    let a = add(&db, "첫째", "가철수", "가영희", "", "");
    add(&db, "둘째", "가철수", "가영희", "", "");
    run_scan(&db);

    let link_id = links(&db, a)[0].link_id;
    db.write(|c| {
        confirm(c, link_id)?;
        student::sync_issues(c, a, 2026, today())
    })
    .unwrap();

    let out = run_scan(&db);
    assert_eq!(out.confirmed, 1);
    assert_eq!(out.new_candidates, 0);
    assert_eq!(links(&db, a)[0].status, "CONFIRMED");
    assert!(issues_of(&db, a, "SIBLING_CANDIDATE").is_empty());
}

#[test]
fn 결정을_물리면_다시_후보가_된다() {
    let db = db();
    let a = add(&db, "첫째", "가철수", "가영희", "", "");
    add(&db, "둘째", "가철수", "가영희", "", "");
    run_scan(&db);

    let link_id = links(&db, a)[0].link_id;
    db.write(|c| reject(c, link_id)).unwrap();
    assert_eq!(links(&db, a)[0].status, "REJECTED");

    db.write(|c| {
        reset(c, link_id)?;
        student::sync_issues(c, a, 2026, today())
    })
    .unwrap();
    assert_eq!(links(&db, a)[0].status, "CANDIDATE");
    assert_eq!(issues_of(&db, a, "SIBLING_CANDIDATE").len(), 1);
}

#[test]
fn 확정하면_양쪽_모두에서_서로가_보인다() {
    let db = db();
    let a = add_at(&db, "첫째", 1, "나리", "가철수", "가영희", "", "");
    let b = add_at(&db, "둘째", 4, "2", "가철수", "가영희", "", "");
    run_scan(&db);

    let link_id = links(&db, a)[0].link_id;
    db.write(|c| confirm(c, link_id)).unwrap();

    assert_eq!(links(&db, a)[0].label, "4-2 둘째");
    assert_eq!(links(&db, b)[0].label, "1-나리 첫째");
}

#[test]
fn 근거가_사라진_후보는_지워진다() {
    let db = db();
    let a = add(&db, "첫째", "가철수", "가영희", "", "");
    let b = add(&db, "둘째", "가철수", "가영희", "", "");
    run_scan(&db);
    assert_eq!(links(&db, a).len(), 1);

    // 부모 이름이 바뀌어 더는 겹치지 않는다
    db.write(|c| {
        c.execute(
            "UPDATE students SET father_name = '다철수', mother_name = '다영희' WHERE id = ?1",
            [b],
        )?;
        Ok(())
    })
    .unwrap();

    let out = run_scan(&db);
    assert_eq!(out.dropped, 1);
    assert!(links(&db, a).is_empty());
    assert!(issues_of(&db, a, "SIBLING_CANDIDATE").is_empty());
}

#[test]
fn 사용자가_정한_관계는_근거가_흐려져도_남는다() {
    let db = db();
    let a = add(&db, "첫째", "가철수", "가영희", "", "");
    let b = add(&db, "둘째", "가철수", "가영희", "", "");
    run_scan(&db);
    let link_id = links(&db, a)[0].link_id;
    db.write(|c| confirm(c, link_id)).unwrap();

    // 한 쪽 부모 이름이 바뀌었다
    db.write(|c| {
        c.execute(
            "UPDATE students SET father_name = '다철수' WHERE id = ?1",
            [b],
        )?;
        Ok(())
    })
    .unwrap();

    run_scan(&db);
    let v = links(&db, a);
    assert_eq!(v.len(), 1, "관계를 저절로 풀면 안 된다");
    assert_eq!(v[0].status, "CONFIRMED");
}

// ---------------------------------------------------------------
// 보호자 정보 — 가져오기
// ---------------------------------------------------------------

/// A 는 부 정보가 비어 있고, B 는 다 채워져 있다. 둘은 모 정보로 형제 후보가 된다.
fn pair_with_missing(db: &Db) -> (i64, i64, i64) {
    let a = add(&db, "빈칸있음", "", "가영희", "", "010-3333-4444");
    let b = add(&db, "다채움", "가철수", "가영희", "010-1111-2222", "010-3333-4444");
    run_scan(db);
    let link_id = links(db, a)[0].link_id;
    db.write(|c| {
        confirm(c, link_id)?;
        student::sync_issues_with_siblings(c, a, 2026, today())
    })
    .unwrap();
    (a, b, link_id)
}

#[test]
fn 확정하면_가져올_수_있는_정보를_알려_준다() {
    let db = db();
    let (a, b, _) = pair_with_missing(&db);

    let v = links(&db, a);
    let keys: Vec<&str> = v[0].fillable.iter().map(|f| f.key.as_str()).collect();
    assert_eq!(keys, vec!["fatherName", "fatherPhone"]);
    assert_eq!(v[0].fillable[0].theirs.as_deref(), Some("가철수"));

    let found = issues_of(&db, a, "GUARDIAN_FILL");
    assert_eq!(found.len(), 1);
    assert!(found[0].message.contains("부 성명"), "{}", found[0].message);

    // 다 채워진 쪽에는 가져올 것이 없다
    assert!(links(&db, b)[0].fillable.is_empty());
    assert!(issues_of(&db, b, "GUARDIAN_FILL").is_empty());
}

#[test]
fn 형제로_확정하기_전에는_가져오기를_권하지_않는다() {
    let db = db();
    let a = add(&db, "빈칸있음", "", "가영희", "", "010-3333-4444");
    add(&db, "다채움", "가철수", "가영희", "010-1111-2222", "010-3333-4444");
    run_scan(&db);

    assert!(links(&db, a)[0].fillable.is_empty(), "후보 단계에서는 권하지 않는다");
    assert!(issues_of(&db, a, "GUARDIAN_FILL").is_empty());
}

#[test]
fn 고른_항목만_가져온다() {
    let db = db();
    let (a, _, link_id) = pair_with_missing(&db);

    let filled = db
        .write(|c| {
            let f = fill_from_sibling(c, link_id, a, &[Field::FatherName])?;
            student::sync_issues_with_siblings(c, a, 2026, today())?;
            Ok(f)
        })
        .unwrap();

    assert_eq!(filled, vec![Field::FatherName]);
    let g = db.read(|c| guardians_of(c, a)).unwrap();
    assert_eq!(g.father_name.as_deref(), Some("가철수"));
    assert_eq!(g.father_phone, None, "고르지 않은 항목은 그대로 비어 있다");

    // 남은 것이 있으므로 표시도 남는다
    assert_eq!(issues_of(&db, a, "GUARDIAN_FILL").len(), 1);
}

#[test]
fn 다_가져오면_표시가_사라진다() {
    let db = db();
    let (a, _, link_id) = pair_with_missing(&db);

    db.write(|c| {
        fill_from_sibling(c, link_id, a, &[Field::FatherName, Field::FatherPhone])?;
        student::sync_issues_with_siblings(c, a, 2026, today())
    })
    .unwrap();

    let g = db.read(|c| guardians_of(c, a)).unwrap();
    assert_eq!(g.father_name.as_deref(), Some("가철수"));
    assert_eq!(g.father_phone.as_deref(), Some("010-1111-2222"), "저장하며 정리된다");
    assert!(issues_of(&db, a, "GUARDIAN_FILL").is_empty());
}

#[test]
fn 값이_있는_항목은_가져오기로_덮어쓰지_않는다() {
    let db = db();
    let a = add(&db, "값있음", "나철수", "가영희", "", "010-3333-4444");
    add(&db, "다름", "가철수", "가영희", "010-1111-2222", "010-3333-4444");
    run_scan(&db);
    let link_id = links(&db, a)[0].link_id;
    db.write(|c| confirm(c, link_id)).unwrap();

    // 부 성명을 억지로 가져오라고 해도 값이 있으므로 채우지 않는다
    let filled = db
        .write(|c| fill_from_sibling(c, link_id, a, &[Field::FatherName, Field::FatherPhone]))
        .unwrap();

    assert_eq!(filled, vec![Field::FatherPhone], "빈칸이던 것만 채운다");
    let g = db.read(|c| guardians_of(c, a)).unwrap();
    assert_eq!(g.father_name.as_deref(), Some("나철수"), "있던 값 그대로");
}

#[test]
fn 형제가_아닌_관계로는_가져오지_못한다() {
    let db = db();
    let a = add(&db, "빈칸있음", "", "가영희", "", "010-3333-4444");
    add(&db, "다채움", "가철수", "가영희", "010-1111-2222", "010-3333-4444");
    run_scan(&db);
    let link_id = links(&db, a)[0].link_id;

    let err = db
        .write(|c| fill_from_sibling(c, link_id, a, &[Field::FatherName]))
        .unwrap_err();
    assert_eq!(err.code, "INVALID_INPUT");
    assert!(err.user_message.contains("형제로 확인"));
}

// ---------------------------------------------------------------
// 보호자 정보 — 불일치
// ---------------------------------------------------------------

#[test]
fn 확정_형제와_값이_다르면_확인을_요청한다() {
    let db = db();
    let a = add(&db, "첫째", "가철수", "가영희", "", "010-1111-2222");
    let b = add(&db, "둘째", "가철수", "가영희", "", "010-3333-4444");
    run_scan(&db);
    let link_id = links(&db, a)[0].link_id;
    db.write(|c| {
        confirm(c, link_id)?;
        student::sync_issues_with_siblings(c, a, 2026, today())
    })
    .unwrap();

    // 관계는 그대로 확정이다
    assert_eq!(links(&db, a)[0].status, "CONFIRMED");

    let found = issues_of(&db, a, "GUARDIAN_CONFLICT");
    assert_eq!(found.len(), 1);
    assert!(found[0].message.contains("모 연락처"), "{}", found[0].message);

    // 양쪽 값을 견줄 수 있어야 한다
    let v = links(&db, a);
    let conflict = &v[0].conflicts[0];
    assert_eq!(conflict.mine.as_deref(), Some("010-1111-2222"));
    assert_eq!(conflict.theirs.as_deref(), Some("010-3333-4444"));

    // 반대쪽에서도 보인다
    assert_eq!(issues_of(&db, b, "GUARDIAN_CONFLICT").len(), 1);
}

#[test]
fn 값이_달라도_형제_관계는_풀리지_않는다() {
    let db = db();
    let a = add(&db, "첫째", "가철수", "가영희", "010-1111-1111", "010-2222-2222");
    add(&db, "둘째", "가철수", "가영희", "010-9999-9999", "010-2222-2222");
    run_scan(&db);
    let link_id = links(&db, a)[0].link_id;
    db.write(|c| confirm(c, link_id)).unwrap();

    run_scan(&db);
    let v = links(&db, a);
    assert_eq!(v[0].status, "CONFIRMED", "연락처가 달라도 형제일 수 있다");
    assert_eq!(v[0].conflicts.len(), 1);
}

#[test]
fn 값을_고쳐_같아지면_확인_요청이_사라진다() {
    let db = db();
    let a = add(&db, "첫째", "가철수", "가영희", "", "010-1111-2222");
    let b = add(&db, "둘째", "가철수", "가영희", "", "010-3333-4444");
    run_scan(&db);
    let link_id = links(&db, a)[0].link_id;
    db.write(|c| {
        confirm(c, link_id)?;
        student::sync_issues_with_siblings(c, a, 2026, today())
    })
    .unwrap();
    assert_eq!(issues_of(&db, a, "GUARDIAN_CONFLICT").len(), 1);
    assert_eq!(issues_of(&db, b, "GUARDIAN_CONFLICT").len(), 1);

    // A 의 연락처를 B 와 같게 고친다 — 학생 수정 경로를 그대로 쓴다
    let input = student::StudentInput {
        name: "첫째".into(),
        gender: Some("M".into()),
        birth_raw: Some("170315".into()),
        father_name: Some("가철수".into()),
        mother_name: Some("가영희".into()),
        mother_phone: Some("010-3333-4444".into()),
        school_year: 2026,
        grade: 3,
        class_name: Some("가람".into()),
        class_no: Some(1),
        ..Default::default()
    };
    db.write(|c| student::update(c, a, &input, today())).unwrap();

    assert!(issues_of(&db, a, "GUARDIAN_CONFLICT").is_empty());
    assert!(
        issues_of(&db, b, "GUARDIAN_CONFLICT").is_empty(),
        "상대 쪽 표시도 함께 닫혀야 한다"
    );
}

#[test]
fn 값을_다르게_고치면_확인_요청이_새로_생긴다() {
    let db = db();
    let a = add(&db, "첫째", "가철수", "가영희", "", "010-1111-2222");
    let b = add(&db, "둘째", "가철수", "가영희", "", "010-1111-2222");
    run_scan(&db);
    let link_id = links(&db, a)[0].link_id;
    db.write(|c| {
        confirm(c, link_id)?;
        student::sync_issues_with_siblings(c, a, 2026, today())
    })
    .unwrap();
    assert!(issues_of(&db, a, "GUARDIAN_CONFLICT").is_empty());

    let input = student::StudentInput {
        name: "첫째".into(),
        gender: Some("M".into()),
        birth_raw: Some("170315".into()),
        father_name: Some("가철수".into()),
        mother_name: Some("가영희".into()),
        mother_phone: Some("010-9999-8888".into()),
        school_year: 2026,
        grade: 3,
        class_name: Some("가람".into()),
        class_no: Some(1),
        ..Default::default()
    };
    db.write(|c| student::update(c, a, &input, today())).unwrap();

    assert_eq!(issues_of(&db, a, "GUARDIAN_CONFLICT").len(), 1);
    assert_eq!(
        issues_of(&db, b, "GUARDIAN_CONFLICT").len(),
        1,
        "상대 쪽에도 생긴다"
    );
}

// ---------------------------------------------------------------
// 여러 형제
// ---------------------------------------------------------------

#[test]
fn 한_학생이_여러_형제를_가질_수_있다() {
    let db = db();
    let a = add(&db, "첫째", "가철수", "가영희", "", "");
    add(&db, "둘째", "가철수", "가영희", "", "");
    add(&db, "셋째", "가철수", "가영희", "", "");
    run_scan(&db);

    for v in links(&db, a) {
        db.write(|c| confirm(c, v.link_id)).unwrap();
    }
    db.write(|c| student::sync_issues_with_siblings(c, a, 2026, today()))
        .unwrap();

    let confirmed = db.read(|c| confirmed_partners(c, a)).unwrap();
    assert_eq!(confirmed.len(), 2);

    // **몇 명인지가 아니라 누구인지**를 보여 준다 — 명단에서 바로 확인할 수 있어야 한다
    let b = db.read(|c| brief(c, a, 2026, today())).unwrap().unwrap();
    assert_eq!(b.count, 2);
    assert_eq!(b.labels, vec!["3-가람 둘째", "3-가람 셋째"]);
    assert_eq!(b.text, "3-가람 둘째 · 3-가람 셋째");
    assert!(!b.text.contains("명"), "'2명' 으로 줄이지 않는다");
}

#[test]
fn 형제가_셋이면_셋_다_보여_준다() {
    let db = db();
    let a = add_at(&db, "첫째", 2, "가람", "가철수", "가영희", "", "");
    add_at(&db, "둘째", 4, "나리", "가철수", "가영희", "", "");
    add_at(&db, "셋째", 6, "다솜", "가철수", "가영희", "", "");
    add_at(&db, "넷째", 1, "라온", "가철수", "가영희", "", "");
    run_scan(&db);

    for v in links(&db, a) {
        db.write(|c| confirm(c, v.link_id)).unwrap();
    }

    let b = db.read(|c| brief(c, a, 2026, today())).unwrap().unwrap();
    assert_eq!(b.count, 3);
    assert_eq!(
        b.text, "1-라온 넷째 · 4-나리 둘째 · 6-다솜 셋째",
        "학년·반 이름표가 모두 들어간다"
    );
}

#[test]
fn 글자_반_이름도_그대로_보여_준다() {
    let db = db();
    let a = add_at(&db, "첫째", 1, "나리", "가철수", "가영희", "", "");
    add_at(&db, "둘째", 5, "가람", "가철수", "가영희", "", "");
    run_scan(&db);
    for v in links(&db, a) {
        db.write(|c| confirm(c, v.link_id)).unwrap();
    }

    let b = db.read(|c| brief(c, a, 2026, today())).unwrap().unwrap();
    assert_eq!(b.text, "5-가람 둘째", "반이 TEXT 라도 그대로");
}

#[test]
fn 반이_없는_형제는_미정으로_보여_준다() {
    // 억지로 숫자를 만들지 않는다 — 지금 자료 그대로 보여 준다
    let db = db();
    let a = add_at(&db, "첫째", 1, "나리", "가철수", "가영희", "", "");
    let b = add_at(&db, "둘째", 3, "가람", "가철수", "가영희", "", "");
    db.write(|c| {
        c.execute(
            "UPDATE enrollments SET class_name = NULL WHERE student_id = ?1",
            [b],
        )?;
        Ok(())
    })
    .unwrap();
    run_scan(&db);
    for v in links(&db, a) {
        db.write(|c| confirm(c, v.link_id)).unwrap();
    }

    let brief = db.read(|c| brief(c, a, 2026, today())).unwrap().unwrap();
    assert_eq!(brief.text, "3-미정 둘째");
}

#[test]
fn 전출한_형제는_여럿일_때도_빠진다() {
    // v0.1.3 의 '현재 본교 형제' 판정은 그대로다 — 전원 표시가 그것을 바꾸지 않는다
    let db = db();
    let a = add_at(&db, "첫째", 2, "가람", "가철수", "가영희", "", "");
    add_at(&db, "둘째", 4, "나리", "가철수", "가영희", "", "");
    let third = add_at(&db, "셋째", 6, "다솜", "가철수", "가영희", "", "");
    add_at(&db, "넷째", 1, "라온", "가철수", "가영희", "", "");
    run_scan(&db);
    for v in links(&db, a) {
        db.write(|c| confirm(c, v.link_id)).unwrap();
    }
    db.write(|c| {
        c.execute(
            "UPDATE enrollments
                SET status = 'TRANSFER_OUT', transfer_out_date = '2026-09-01'
              WHERE student_id = ?1",
            [third],
        )?;
        Ok(())
    })
    .unwrap();

    let b = db.read(|c| brief(c, a, 2026, today())).unwrap().unwrap();
    assert_eq!(b.count, 2, "전출한 셋째는 본교 형제가 아니다");
    assert_eq!(b.text, "1-라온 넷째 · 4-나리 둘째");
}

#[test]
fn 형제가_한_명이면_이름표로_보여_준다() {
    let db = db();
    let a = add_at(&db, "첫째", 1, "나리", "가철수", "가영희", "", "");
    add_at(&db, "둘째", 2, "가람", "가철수", "가영희", "", "");
    run_scan(&db);
    let link_id = links(&db, a)[0].link_id;
    db.write(|c| confirm(c, link_id)).unwrap();

    let b = db.read(|c| brief(c, a, 2026, today())).unwrap().unwrap();
    assert_eq!(b.count, 1);
    assert_eq!(b.text, "2-가람 둘째");
}

#[test]
fn 확정_형제가_없으면_표시할_것이_없다() {
    let db = db();
    let a = add(&db, "혼자", "가철수", "가영희", "", "");
    assert!(db.read(|c| brief(c, a, 2026, today())).unwrap().is_none());
}

#[test]
fn 상대마다_확인_필요가_따로_생기고_따로_닫힌다() {
    let db = db();
    let a = add(&db, "첫째", "가철수", "가영희", "", "");
    add(&db, "둘째", "가철수", "가영희", "", "");
    add(&db, "셋째", "가철수", "가영희", "", "");
    run_scan(&db);

    assert_eq!(issues_of(&db, a, "SIBLING_CANDIDATE").len(), 2);

    // 하나만 결정한다
    let first = links(&db, a)[0].link_id;
    db.write(|c| {
        confirm(c, first)?;
        student::sync_issues(c, a, 2026, today())
    })
    .unwrap();

    assert_eq!(
        issues_of(&db, a, "SIBLING_CANDIDATE").len(),
        1,
        "결정한 것만 닫히고 나머지는 남는다"
    );
}

#[test]
fn 셋_이상_형제의_보호자_정보는_관계마다_따로_본다() {
    // A-B, A-C 는 확정. B 와 C 의 연락처가 서로 달라도 프로그램이 정답을 만들지 않는다.
    let db = db();
    let a = add(&db, "첫째", "가철수", "가영희", "", "");
    let b = add(&db, "둘째", "가철수", "가영희", "", "010-1111-1111");
    let c = add(&db, "셋째", "가철수", "가영희", "", "010-2222-2222");
    run_scan(&db);
    for v in links(&db, a) {
        db.write(|x| confirm(x, v.link_id)).unwrap();
    }
    db.write(|x| student::sync_issues_with_siblings(x, a, 2026, today()))
        .unwrap();

    // A 는 비어 있으므로 양쪽에서 가져올 수 있다고 알려 준다 (관계마다 한 줄)
    assert_eq!(issues_of(&db, a, "GUARDIAN_FILL").len(), 2);
    // B 와 C 는 서로 확정 관계가 아니므로 둘 사이 불일치는 따지지 않는다
    assert!(issues_of(&db, b, "GUARDIAN_CONFLICT").is_empty());
    assert!(issues_of(&db, c, "GUARDIAN_CONFLICT").is_empty());
}

// ---------------------------------------------------------------
// 많은 학생
// ---------------------------------------------------------------

#[test]
fn 천명_넘는_학생도_빠르게_훑는다() {
    let db = db();
    // 1,200명. 400집에 세 명씩 — 형제 쌍이 1,200개 나온다.
    db.write(|c| {
        for i in 0..1200 {
            let family = i / 3;
            c.execute(
                "INSERT INTO students(name, father_name, mother_name, father_phone, father_phone_digits)
                 VALUES (?1,?2,?3,?4,?5)",
                params![
                    format!("학생{i:04}"),
                    format!("아버지{family}"),
                    format!("어머니{family}"),
                    format!("010-{:04}-{:04}", family, family),
                    format!("010{:04}{:04}", family, family),
                ],
            )?;
            let id = c.last_insert_rowid();
            c.execute(
                "INSERT INTO enrollments(student_id, school_year, grade, class_name, class_no)
                 VALUES (?1, 2026, ?2, '가람', ?3)",
                params![id, (i % 6) + 1, (i % 30) + 1],
            )?;
        }
        Ok(())
    })
    .unwrap();

    let started = std::time::Instant::now();
    let out = db.write(|c| scan(c, 2026, today(), |_, _, _| {})).unwrap();
    let took = started.elapsed();

    assert_eq!(out.scanned, 1200);
    assert_eq!(out.new_candidates, 1200, "400집 × 3쌍");
    assert!(took.as_secs() < 20, "{took:?} 걸렸다");
}

#[test]
fn 빈칸이_많아도_후보가_불어나지_않는다() {
    let db = db();
    db.write(|c| {
        for i in 0..500 {
            c.execute("INSERT INTO students(name) VALUES (?1)", [format!("학생{i}")])?;
            let id = c.last_insert_rowid();
            c.execute(
                "INSERT INTO enrollments(student_id, school_year, grade, class_name, class_no)
                 VALUES (?1, 2026, 1, '가람', ?2)",
                params![id, (i % 30) + 1],
            )?;
        }
        Ok(())
    })
    .unwrap();

    let out = db.write(|c| scan(c, 2026, today(), |_, _, _| {})).unwrap();
    assert_eq!(out.scanned, 500);
    assert_eq!(out.new_candidates, 0, "보호자 정보가 없는 학생끼리 묶이면 안 된다");
}

#[test]
fn 진행_상황을_단계별로_알려_준다() {
    let db = db();
    add(&db, "가", "가철수", "가영희", "", "");
    add(&db, "나", "가철수", "가영희", "", "");

    let mut seen: Vec<(String, usize, usize)> = Vec::new();
    db.write(|c| {
        scan(c, 2026, today(), |stage, done, total| {
            seen.push((stage.to_string(), done, total))
        })
    })
    .unwrap();

    let stages: Vec<&str> = seen.iter().map(|(s, _, _)| s.as_str()).collect();
    assert!(stages.contains(&"PAIR"), "{stages:?}");
    assert!(stages.contains(&"COMPARE"), "{stages:?}");
    assert!(stages.contains(&"GUARDIAN"), "{stages:?}");
    assert!(seen.iter().all(|(_, d, t)| d <= t));
}

// ---------------------------------------------------------------
// 후보 한꺼번에 확정하기
// ---------------------------------------------------------------

/// 후보 한 쌍씩 `n` 쌍. 쌍마다 보호자 이름이 달라 서로 얽히지 않는다.
fn pairs(db: &Db, n: usize) -> Vec<(i64, i64)> {
    let mut out = Vec::new();
    for i in 0..n {
        let f = format!("남궁바다{i}");
        let m = format!("제갈하늘{i}");
        let a = add(db, &format!("가온{i}"), &f, &m, "", "");
        let b = add(db, &format!("해솔{i}"), &f, &m, "", "");
        out.push((a, b));
    }
    run_scan(db);
    out
}

fn candidate_rows(db: &Db) -> Vec<CandidateRow> {
    db.read(|c| candidates(c, 2026, today())).unwrap()
}

/// 명령이 하는 일과 같다 — 한 트랜잭션에서 확정하고 표시를 다시 맞춘다.
fn batch(db: &Db, ids: &[i64]) -> AppResult<BatchConfirm> {
    db.write(|c| {
        let out = confirm_many(c, ids)?;
        for id in &out.students {
            student::sync_issues(c, *id, 2026, today())?;
        }
        Ok(out)
    })
}

fn status_of(db: &Db, link_id: i64) -> String {
    db.read(|c| {
        Ok(c.query_row(
            "SELECT status FROM sibling_links WHERE id = ?1",
            [link_id],
            |r| r.get::<_, String>(0),
        )?)
    })
    .unwrap()
}

#[test]
fn 후보_열_건을_한_번에_형제로_확정한다() {
    let db = db();
    let made = pairs(&db, 10);

    let rows = candidate_rows(&db);
    assert_eq!(rows.len(), 10, "쌍마다 한 줄이다 (학생마다가 아니다)");
    assert!(
        rows.iter().all(|r| r.matched.contains(&"부 성명".to_string())
            && r.matched.contains(&"모 성명".to_string())),
        "무엇이 같아서 후보인지 함께 보여 준다"
    );

    let ids: Vec<i64> = rows.iter().map(|r| r.link_id).collect();
    let out = batch(&db, &ids).unwrap();
    assert_eq!(out.confirmed, 10);
    assert_eq!(out.already, 0);
    assert_eq!(out.skipped, 0);
    assert_eq!(out.students.len(), 20);

    for id in &ids {
        assert_eq!(status_of(&db, *id), "CONFIRMED");
    }
    for (a, b) in &made {
        assert!(issues_of(&db, *a, "SIBLING_CANDIDATE").is_empty(), "표시가 남았다");
        assert!(issues_of(&db, *b, "SIBLING_CANDIDATE").is_empty());
    }
    assert!(candidate_rows(&db).is_empty(), "확인 필요 목록에서 사라진다");
}

#[test]
fn 고른_후보만_확정하고_나머지는_후보로_남는다() {
    let db = db();
    pairs(&db, 10);

    let rows = candidate_rows(&db);
    let picked: Vec<i64> = rows.iter().take(8).map(|r| r.link_id).collect();
    let left: Vec<i64> = rows.iter().skip(8).map(|r| r.link_id).collect();

    let out = batch(&db, &picked).unwrap();
    assert_eq!(out.confirmed, 8);

    for id in &picked {
        assert_eq!(status_of(&db, *id), "CONFIRMED");
    }
    for id in &left {
        assert_eq!(status_of(&db, *id), "CANDIDATE", "고르지 않은 것은 그대로다");
    }
    assert_eq!(candidate_rows(&db).len(), 2);
}

#[test]
fn 하나라도_없는_후보면_아무것도_확정하지_않는다() {
    let db = db();
    pairs(&db, 10);

    let rows = candidate_rows(&db);
    let mut ids: Vec<i64> = rows.iter().map(|r| r.link_id).collect();
    ids.insert(5, 999_999); // 목록을 띄워 둔 사이에 사라진 관계

    let err = batch(&db, &ids).unwrap_err();
    assert_eq!(err.code, "NOT_FOUND");

    for r in &rows {
        assert_eq!(
            status_of(&db, r.link_id),
            "CANDIDATE",
            "절반만 확정되면 안 된다"
        );
    }
    assert_eq!(candidate_rows(&db).len(), 10);
}

#[test]
fn 형제_아님은_일괄_확정으로도_되살아나지_않는다() {
    let db = db();
    pairs(&db, 3);

    let rows = candidate_rows(&db);
    let rejected = rows[0].link_id;
    db.write(|c| {
        reject(c, rejected)?;
        Ok(())
    })
    .unwrap();

    let ids: Vec<i64> = rows.iter().map(|r| r.link_id).collect();
    let out = batch(&db, &ids).unwrap();
    assert_eq!(out.confirmed, 2);
    assert_eq!(out.skipped, 1);
    assert_eq!(status_of(&db, rejected), "REJECTED", "사용자 결정이 먼저다");
}

#[test]
fn 이미_확정한_관계를_다시_보내도_오류가_아니다() {
    let db = db();
    pairs(&db, 3);
    let ids: Vec<i64> = candidate_rows(&db).iter().map(|r| r.link_id).collect();

    batch(&db, &ids).unwrap();
    let again = batch(&db, &ids).unwrap();
    assert_eq!(again.confirmed, 0);
    assert_eq!(again.already, 3);
    assert_eq!(again.skipped, 0);
}

#[test]
fn 같은_후보를_두_번_보내도_한_번만_센다() {
    let db = db();
    pairs(&db, 2);
    let ids: Vec<i64> = candidate_rows(&db).iter().map(|r| r.link_id).collect();
    let doubled: Vec<i64> = ids.iter().chain(ids.iter()).copied().collect();

    let out = batch(&db, &doubled).unwrap();
    assert_eq!(out.confirmed, 2);
    assert_eq!(out.already, 0);
}

#[test]
fn 일괄_확정하면_보호자_보완과_불일치가_그대로_따라온다() {
    let db = db();
    // 이름 둘이 같아 후보 — 모 연락처는 한쪽에만 있고(보완), 부 연락처는 서로 다르다(불일치)
    let a = add(&db, "가온", "남궁바다", "제갈하늘", "010-1000-0001", "010-1000-0002");
    let b = add(&db, "해솔", "남궁바다", "제갈하늘", "010-1000-0009", "");
    run_scan(&db);

    let rows = candidate_rows(&db);
    assert_eq!(rows.len(), 1);
    assert!(
        rows[0].conflicts.contains(&"부 연락처".to_string()),
        "확정 전에 다른 값이 있다고 알려 준다"
    );

    // 확정 전에는 형제 관계가 아니므로 보완·불일치 표시가 없다
    assert!(issues_of(&db, b, "GUARDIAN_FILL").is_empty());
    assert!(issues_of(&db, a, "GUARDIAN_CONFLICT").is_empty());

    batch(&db, &[rows[0].link_id]).unwrap();

    assert_eq!(issues_of(&db, b, "GUARDIAN_FILL").len(), 1, "모 연락처를 가져올 수 있다");
    assert!(issues_of(&db, a, "GUARDIAN_FILL").is_empty(), "빈칸이 없는 쪽은 없다");
    assert_eq!(issues_of(&db, a, "GUARDIAN_CONFLICT").len(), 1);
    assert_eq!(issues_of(&db, b, "GUARDIAN_CONFLICT").len(), 1);
    assert!(issues_of(&db, a, "SIBLING_CANDIDATE").is_empty());
}

#[test]
fn 후보_목록에는_확정_형제와_형제_아님이_나오지_않는다() {
    let db = db();
    pairs(&db, 3);
    let rows = candidate_rows(&db);

    db.write(|c| {
        confirm(c, rows[0].link_id)?;
        reject(c, rows[1].link_id)?;
        Ok(())
    })
    .unwrap();
    // 확인 필요 표시도 명령과 같은 자리에서 다시 맞춘다
    db.write(|c| {
        for r in &rows {
            student::sync_issues(c, r.student_a, 2026, today())?;
            student::sync_issues(c, r.student_b, 2026, today())?;
        }
        Ok(())
    })
    .unwrap();

    let left = candidate_rows(&db);
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].link_id, rows[2].link_id);
}

#[test]
fn 후보_목록은_전출한_학생의_쌍을_보여_주지_않는다() {
    let db = db();
    let a = add(&db, "가온", "남궁바다", "제갈하늘", "", "");
    let b = add(&db, "해솔", "남궁바다", "제갈하늘", "", "");
    run_scan(&db);
    assert_eq!(candidate_rows(&db).len(), 1);

    db.write(|c| {
        c.execute(
            "UPDATE enrollments SET status = 'TRANSFER_OUT'
              WHERE student_id IN (?1, ?2) AND school_year = 2026",
            params![a, b],
        )?;
        student::sync_issues(c, a, 2026, today())?;
        student::sync_issues(c, b, 2026, today())
    })
    .unwrap();

    assert!(
        candidate_rows(&db).is_empty(),
        "업무함에 없는 학생은 일괄 확정 목록에도 없다"
    );
}

// ---------------------------------------------------------------
// 지난 학년도 형제 — 확정·거절이 막히면 안 된다
// ---------------------------------------------------------------

/// 지난 학년도에만 학적이 있는 학생을 하나 만들고 후보로 걸어 둔다.
fn with_last_year_candidate(db: &Db, me: i64) -> i64 {
    let old = db
        .write(|c| {
            settings::create_year(c, 2025)?;
            student::create(
                c,
                &student::StudentInput {
                    name: "지난해".into(),
                    father_name: Some("가철수".into()),
                    mother_name: Some("가영희".into()),
                    school_year: 2025,
                    grade: 6,
                    class_name: Some("가람".into()),
                    class_no: Some(1),
                    ..Default::default()
                },
                today(),
            )
        })
        .unwrap();
    db.write(|c| {
        c.execute(
            "INSERT INTO sibling_links(student_a, student_b, status, source)
             VALUES (?1, ?2, 'CANDIDATE', 'MANUAL')",
            [me.min(old), me.max(old)],
        )?;
        Ok(c.last_insert_rowid())
    })
    .unwrap()
}

#[test]
fn 지난_학년도_학생과의_후보도_확정할_수_있다() {
    // 명령이 하는 일 그대로 — 확정한 뒤 두 학생의 표시를 맞춘다.
    // 상대에게 올해 학적이 없다고 `sync_issues` 가 오류를 내면 트랜잭션이 통째로
    // 되돌아가 확정 자체가 안 된다(v0.1.3 에서 실제로 그랬다).
    let db = db();
    let me = add(&db, "첫째", "가철수", "가영희", "", "");
    let link_id = with_last_year_candidate(&db, me);

    db.write(|c| {
        let (a, b) = confirm(c, link_id)?;
        student::sync_issues(c, a, 2026, today())?;
        student::sync_issues(c, b, 2026, today())?;
        Ok(())
    })
    .unwrap();

    assert_eq!(status_of(&db, link_id), "CONFIRMED");
}

#[test]
fn 지난_학년도_학생과의_후보도_거절할_수_있다() {
    let db = db();
    let me = add(&db, "첫째", "가철수", "가영희", "", "");
    let link_id = with_last_year_candidate(&db, me);

    db.write(|c| {
        let (a, b) = reject(c, link_id)?;
        student::sync_issues(c, a, 2026, today())?;
        student::sync_issues(c, b, 2026, today())?;
        Ok(())
    })
    .unwrap();

    assert_eq!(status_of(&db, link_id), "REJECTED");
}

#[test]
fn 지난_학년도_학생은_올해_형제_수에_들지_않는다() {
    // 확정은 되지만 '본교 형제' 는 지금 함께 다니는 학생만 센다
    let db = db();
    let me = add(&db, "첫째", "가철수", "가영희", "", "");
    let link_id = with_last_year_candidate(&db, me);
    db.write(|c| confirm(c, link_id).map(|_| ())).unwrap();

    assert_eq!(db.read(|c| brief(c, me, 2026, today())).unwrap().is_none(), true);
}
