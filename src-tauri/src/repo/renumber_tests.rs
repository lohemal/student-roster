//! 번호 재정렬 저장소 검사.
//!
//! 여기서 보는 것은 세 가지다.
//!   * 미리보기와 저장 결과가 **정확히 같은가**
//!   * 다른 반 학생이 **하나도 안 바뀌는가**
//!   * 오래된 미리보기를 들고 왔을 때 **거절하는가**
//!
//! 시험에 쓰는 이름은 모두 가상이다.

use super::*;
use crate::db::Db;
use crate::repo::{settings, student};

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

/// 학생 하나를 넣는다. 번호가 없으면 `None`.
fn add(db: &Db, name: &str, grade: i32, class_name: &str, class_no: Option<i32>) -> i64 {
    let input = student::StudentInput {
        name: name.into(),
        gender: Some("F".into()),
        birth_raw: Some("170315".into()),
        address_raw: Some("○○시 가온로 101".into()),
        mother_phone: Some("010-0000-0001".into()),
        school_year: 2026,
        grade,
        class_name: Some(class_name.into()),
        class_no,
        ..Default::default()
    };
    db.write(|c| student::create(c, &input, today())).unwrap()
}

/// `1반 학생01 … 학생NN` 을 1번부터 차례로 넣고 학생 번호 목록을 돌려준다.
fn make_class(db: &Db, grade: i32, class_name: &str, n: i32) -> Vec<i64> {
    (1..=n)
        .map(|i| add(db, &format!("학생{i:02}"), grade, class_name, Some(i)))
        .collect()
}

/// 그 반의 (번호, 학생 id) 를 번호순으로
fn seating(db: &Db, grade: i32, class_name: &str) -> Vec<(Option<i32>, i64)> {
    db.read(|c| {
        let mut st = c.prepare(
            "SELECT class_no, student_id FROM enrollments
              WHERE school_year = 2026 AND grade = ?1 AND class_name = ?2
              ORDER BY CASE WHEN class_no IS NULL THEN 1 ELSE 0 END, class_no, student_id",
        )?;
        let out: Vec<(Option<i32>, i64)> = st
            .query_map(params![grade, class_name], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(out)
    })
    .unwrap()
}

fn no_of(db: &Db, student_id: i64) -> Option<i32> {
    db.read(|c| {
        Ok(c.query_row(
            "SELECT class_no FROM enrollments WHERE student_id = ?1 AND school_year = 2026",
            [student_id],
            |r| r.get(0),
        )?)
    })
    .unwrap()
}

fn see(db: &Db, student_id: i64, new_no: i32) -> Preview {
    db.read(|c| preview(c, student_id, 2026, new_no, today())).unwrap()
}

/// 미리보기를 받아 그대로 적용한다 — 화면이 하는 일과 같은 차례.
fn do_move(db: &Db, student_id: i64, new_no: i32) -> AppResult<ApplyResult> {
    let key = see(db, student_id, new_no).state_key;
    db.write(|c| apply(c, student_id, 2026, new_no, &key, today()))
}

fn open_kinds(db: &Db, kind: &str) -> i64 {
    let kind = kind.to_string();
    db.read(|c| {
        Ok(c.query_row(
            "SELECT COUNT(*) FROM issues WHERE status = 'OPEN' AND kind = ?1",
            [kind],
            |r| r.get(0),
        )?)
    })
    .unwrap()
}

// ---------------------------------------------------------------
// 뒤 → 앞 · 앞 → 뒤
// ---------------------------------------------------------------

#[test]
fn 스물세번을_칠번으로_옮긴다() {
    let db = db();
    let ids = make_class(&db, 3, "나리", 23);
    let last = ids[22];

    let p = see(&db, last, 7);
    assert_eq!(p.kind, "REORDER");
    assert_eq!(p.affected, 16);
    assert_eq!(p.class_label, "3-나리");
    assert!(p.blocked.is_none());

    let out = do_move(&db, last, 7).unwrap();
    assert_eq!(out.moved, 17, "대상 1명 + 밀린 16명");

    assert_eq!(no_of(&db, last), Some(7));
    for (i, id) in ids.iter().enumerate().take(6) {
        assert_eq!(no_of(&db, *id), Some(i as i32 + 1), "앞쪽은 그대로다");
    }
    for (i, id) in ids.iter().enumerate().take(22).skip(6) {
        assert_eq!(no_of(&db, *id), Some(i as i32 + 2), "7~22번이 한 칸씩 밀린다");
    }
}

#[test]
fn 칠번을_스물세번으로_옮긴다() {
    let db = db();
    let ids = make_class(&db, 3, "나리", 23);
    let seventh = ids[6];

    do_move(&db, seventh, 23).unwrap();

    assert_eq!(no_of(&db, seventh), Some(23));
    for (i, id) in ids.iter().enumerate().skip(7) {
        assert_eq!(no_of(&db, *id), Some(i as i32), "8~23번이 한 칸씩 당겨진다");
    }
}

#[test]
fn 미리보기와_저장_결과가_같다() {
    let db = db();
    let ids = make_class(&db, 3, "나리", 23);

    let p = see(&db, ids[22], 7);
    let planned: Vec<(i64, i32)> = p.rows.iter().map(|r| (r.student_id, r.to)).collect();

    do_move(&db, ids[22], 7).unwrap();

    for (id, expected) in planned {
        assert_eq!(no_of(&db, id), Some(expected), "미리 보여 준 대로 저장된다");
    }
}

#[test]
fn 같은_번호로_바꾸면_아무것도_저장하지_않는다() {
    let db = db();
    let ids = make_class(&db, 3, "나리", 10);

    let p = see(&db, ids[6], 7);
    assert_eq!(p.kind, "NONE");
    assert!(p.rows.is_empty());

    let out = do_move(&db, ids[6], 7).unwrap();
    assert_eq!(out.moved, 0);
    assert!(out.op_id.is_none(), "기록도 남기지 않는다");
}

// ---------------------------------------------------------------
// 다른 반
// ---------------------------------------------------------------

#[test]
fn 다른_반_학생은_절대_바뀌지_않는다() {
    let db = db();
    let nari = make_class(&db, 3, "나리", 10);
    let garam = make_class(&db, 3, "가람", 10);
    let other_grade = make_class(&db, 5, "나리", 10);

    let before_garam = seating(&db, 3, "가람");
    let before_grade5 = seating(&db, 5, "나리");

    do_move(&db, nari[9], 1).unwrap();

    assert_eq!(seating(&db, 3, "가람"), before_garam, "같은 학년 다른 반");
    assert_eq!(seating(&db, 5, "나리"), before_grade5, "같은 이름 다른 학년");
    assert_eq!(garam.len(), 10);
    assert_eq!(other_grade.len(), 10);
}

// ---------------------------------------------------------------
// 비연속 · 번호 없음
// ---------------------------------------------------------------

#[test]
fn 띄엄띄엄한_번호를_일번부터_다시_매기지_않는다() {
    let db = db();
    let a = add(&db, "학생가", 3, "나리", Some(1));
    let b = add(&db, "학생나", 3, "나리", Some(2));
    let c = add(&db, "학생다", 3, "나리", Some(3));
    let d = add(&db, "학생라", 3, "나리", Some(5));
    let e = add(&db, "학생마", 3, "나리", Some(6));
    let f = add(&db, "학생바", 3, "나리", Some(9));

    do_move(&db, f, 3).unwrap();

    assert_eq!(no_of(&db, a), Some(1));
    assert_eq!(no_of(&db, b), Some(2));
    assert_eq!(no_of(&db, f), Some(3));
    assert_eq!(no_of(&db, c), Some(5), "없던 4번이 생기지 않는다");
    assert_eq!(no_of(&db, d), Some(6));
    assert_eq!(no_of(&db, e), Some(9));
}

#[test]
fn 빈_번호로_옮기면_그_학생만_바뀐다() {
    let db = db();
    let ids = make_class(&db, 3, "나리", 5);
    let before = seating(&db, 3, "나리");

    let p = see(&db, ids[4], 9);
    assert_eq!(p.kind, "ASSIGN");
    assert_eq!(p.affected, 0);

    do_move(&db, ids[4], 9).unwrap();

    assert_eq!(no_of(&db, ids[4]), Some(9));
    for (i, id) in ids.iter().enumerate().take(4) {
        assert_eq!(no_of(&db, *id), before[i].0);
    }
}

#[test]
fn 번호가_없는_학생은_다른_학생이_움직일_때_그대로다() {
    let db = db();
    let ids = make_class(&db, 3, "나리", 5);
    let nobody = add(&db, "학생번호없음", 3, "나리", None);

    do_move(&db, ids[4], 1).unwrap();

    assert_eq!(no_of(&db, nobody), None, "번호가 없는 학생은 밀리지 않는다");
}

#[test]
fn 번호가_없는_학생을_쓰는_번호에_넣으면_뒤가_밀린다() {
    let db = db();
    let ids = make_class(&db, 3, "나리", 5);
    let nobody = add(&db, "학생번호없음", 3, "나리", None);

    let p = see(&db, nobody, 3);
    assert_eq!(p.kind, "INSERT");
    assert_eq!(p.from, None);
    assert_eq!(p.affected, 3, "3·4·5번이 밀린다");

    do_move(&db, nobody, 3).unwrap();

    assert_eq!(no_of(&db, nobody), Some(3));
    assert_eq!(no_of(&db, ids[0]), Some(1));
    assert_eq!(no_of(&db, ids[1]), Some(2));
    assert_eq!(no_of(&db, ids[2]), Some(4));
    assert_eq!(no_of(&db, ids[3]), Some(5));
    assert_eq!(no_of(&db, ids[4]), Some(6));
}

// ---------------------------------------------------------------
// 막아야 하는 것
// ---------------------------------------------------------------

#[test]
fn 번호가_겹치는_반에서는_자동_이동을_막는다() {
    let db = db();
    let ids = make_class(&db, 3, "나리", 5);
    let twin = add(&db, "학생겹침", 3, "나리", Some(3)); // 3번이 둘

    let p = see(&db, ids[4], 1);
    let why = p.blocked.expect("막혀야 한다");
    assert!(why.contains("3-나리"), "어느 반인지 알려 준다");
    assert!(why.contains('3'), "몇 번이 겹치는지 알려 준다");
    assert!(p.rows.is_empty(), "막혔으면 계획을 보여 주지 않는다");

    let err = do_move(&db, ids[4], 1).unwrap_err();
    assert_eq!(err.code, "INVALID_INPUT");
    assert_eq!(seating(&db, 3, "나리").len(), 6);
    assert_eq!(no_of(&db, ids[4]), Some(5), "하나도 바뀌지 않았다");
    assert_eq!(no_of(&db, twin), Some(3));
}

#[test]
fn 쓸_수_없는_번호는_막는다() {
    let db = db();
    let ids = make_class(&db, 3, "나리", 5);

    for bad in [0, -1, 201] {
        let p = see(&db, ids[0], bad);
        assert!(p.blocked.is_some(), "{bad} 는 막혀야 한다");
        assert!(do_move(&db, ids[0], bad).is_err());
    }
    assert_eq!(no_of(&db, ids[0]), Some(1));
}

#[test]
fn 오래된_미리보기는_적용하지_않는다() {
    let db = db();
    let ids = make_class(&db, 3, "나리", 10);

    // 미리보기를 받아 둔다
    let stale = see(&db, ids[9], 3).state_key;

    // 그 사이 다른 사람이 번호를 고쳤다
    do_move(&db, ids[8], 1).unwrap();

    let err = db
        .write(|c| apply(c, ids[9], 2026, 3, &stale, today()))
        .unwrap_err();
    assert_eq!(err.code, "STALE");

    // 새로 미리보기를 받으면 된다
    do_move(&db, ids[9], 3).unwrap();
    assert_eq!(no_of(&db, ids[9]), Some(3));
}

#[test]
fn 학생이_늘어도_오래된_미리보기는_거절한다() {
    let db = db();
    let ids = make_class(&db, 3, "나리", 5);
    let stale = see(&db, ids[4], 1).state_key;

    add(&db, "학생전입", 3, "나리", Some(6));

    let err = db
        .write(|c| apply(c, ids[4], 2026, 1, &stale, today()))
        .unwrap_err();
    assert_eq!(err.code, "STALE");
}

#[test]
fn 실패하면_한_명도_바뀌지_않는다() {
    let db = db();
    let ids = make_class(&db, 3, "나리", 10);
    let before = seating(&db, 3, "나리");

    // 트랜잭션 안에서 번호를 바꾼 뒤 일부러 실패시킨다
    let err = db
        .write::<()>(|c| {
            let key = renumber::state_key(&seats_of(&rows_of(
                c,
                2026,
                &ClassRef {
                    grade: 3,
                    class_name: Some("나리".into()),
                },
                today(),
            )?));
            apply(c, ids[9], 2026, 1, &key, today())?;
            Err(AppError::internal("일부러 낸 오류"))
        })
        .unwrap_err();
    assert_eq!(err.code, "INTERNAL");

    assert_eq!(seating(&db, 3, "나리"), before, "통째로 되돌아간다");
}

// ---------------------------------------------------------------
// 기록과 표시
// ---------------------------------------------------------------

#[test]
fn 재정렬_작업을_한_줄_남긴다() {
    let db = db();
    let ids = make_class(&db, 3, "나리", 23);
    do_move(&db, ids[22], 7).unwrap();

    let (grade, class_name, from_no, to_no, kind, moved): (
        i32,
        Option<String>,
        Option<i32>,
        i32,
        String,
        i64,
    ) = db
        .read(|c| {
            Ok(c.query_row(
                "SELECT grade, class_name, from_no, to_no, kind, moved FROM renumber_ops",
                [],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                    ))
                },
            )?)
        })
        .unwrap();

    assert_eq!((grade, class_name.as_deref()), (3, Some("나리")));
    assert_eq!((from_no, to_no), (Some(23), 7));
    assert_eq!(kind, "REORDER");
    assert_eq!(moved, 17, "한 번의 작업으로 몇 명이 바뀌었는지 남는다");
}

#[test]
fn 번호_중복은_확인_필요로_표시되고_정리하면_사라진다() {
    let db = db();
    let ids = make_class(&db, 3, "나리", 5);
    let twin = add(&db, "학생겹침", 3, "나리", Some(3));

    assert_eq!(open_kinds(&db, "NUMBER_DUP"), 2, "겹친 두 학생 모두에게 표시한다");

    // 겹친 학생을 빈 번호로 옮기면 양쪽 표시가 모두 사라져야 한다
    let input = student::StudentInput {
        name: "학생겹침".into(),
        gender: Some("F".into()),
        birth_raw: Some("170315".into()),
        address_raw: Some("○○시 가온로 101".into()),
        mother_phone: Some("010-0000-0001".into()),
        school_year: 2026,
        grade: 3,
        class_name: Some("나리".into()),
        class_no: Some(9),
        ..Default::default()
    };
    db.write(|c| student::update(c, twin, &input, today())).unwrap();

    assert_eq!(open_kinds(&db, "NUMBER_DUP"), 0, "상대 쪽 표시도 함께 닫힌다");
    assert_eq!(no_of(&db, ids[2]), Some(3));
}

#[test]
fn 번호를_옮긴_뒤_확인_필요를_다시_따진다() {
    let db = db();
    let ids = make_class(&db, 3, "나리", 5);

    // 번호가 겹치지 않는 정상 반이므로 옮긴 뒤에도 표시가 없어야 한다
    do_move(&db, ids[4], 1).unwrap();
    assert_eq!(open_kinds(&db, "NUMBER_DUP"), 0);
    assert_eq!(open_kinds(&db, "CLASS_ASSIGN"), 0);
    assert_eq!(seating(&db, 3, "나리").len(), 5);
}

#[test]
fn 전출_학생은_번호_계산에_들어오지_않는다() {
    let db = db();
    let ids = make_class(&db, 3, "나리", 5);

    db.write(|c| {
        Ok(c.execute(
            "UPDATE enrollments SET status = 'TRANSFER_OUT'
              WHERE student_id = ?1 AND school_year = 2026",
            [ids[2]],
        )?)
    })
    .unwrap();

    let p = see(&db, ids[4], 1);
    assert!(
        p.rows.iter().all(|r| r.student_id != ids[2]),
        "전출한 학생은 밀지 않는다"
    );

    do_move(&db, ids[4], 1).unwrap();
    assert_eq!(no_of(&db, ids[2]), Some(3), "전출 학생 번호는 그대로다");
}

#[test]
fn 번호가_겹쳐도_빈_번호로_비켜서면_중복이_풀린다() {
    let db = db();
    let ids = make_class(&db, 3, "나리", 5);
    let twin = add(&db, "학생겹침", 3, "나리", Some(3)); // 3번이 둘
    assert_eq!(open_kinds(&db, "NUMBER_DUP"), 2);

    // 9번은 비어 있다 — 아무도 밀지 않으므로 막을 까닭이 없다
    let p = see(&db, twin, 9);
    assert!(p.blocked.is_none(), "빠져나갈 길을 막으면 안 된다");
    assert_eq!(p.kind, "ASSIGN");

    do_move(&db, twin, 9).unwrap();

    assert_eq!(no_of(&db, twin), Some(9));
    assert_eq!(no_of(&db, ids[2]), Some(3), "남은 학생은 3번 그대로");
    assert_eq!(open_kinds(&db, "NUMBER_DUP"), 0, "양쪽 표시가 모두 닫힌다");

    // 중복이 풀렸으니 이제 순서를 지키는 이동도 된다
    assert!(see(&db, twin, 2).blocked.is_none());
}
