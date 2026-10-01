//! 전입·전출 검사.
//!
//! 가장 중요한 것은 **같은 학년도에 나갔다 돌아오는 경우**다. 학적을 하나 더 만들면
//! 한 사람이 두 줄이 되고, 이력을 덮어쓰면 언제 나갔는지가 사라진다.
//! 이름·학교 이름은 모두 가상이다.

use super::*;
use crate::db::Db;
use crate::repo::{issue, settings, stats};

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

fn input(name: &str, year: i32, grade: i32, class_name: &str, no: Option<i32>) -> StudentInput {
    StudentInput {
        name: name.into(),
        gender: Some("M".into()),
        birth_raw: Some("170315".into()),
        address_raw: Some("○○시 가온로 101".into()),
        mother_phone: Some("010-0000-0001".into()),
        school_year: year,
        grade,
        class_name: Some(class_name.into()),
        class_no: no,
        ..Default::default()
    }
}

/// 그냥 다니는 학생 하나
fn enroll(db: &Db, name: &str, grade: i32, class_name: &str, no: i32) -> i64 {
    let i = input(name, 2026, grade, class_name, Some(no));
    db.write(|c| student::create(c, &i, today())).unwrap()
}

fn come_in(db: &Db, id: Option<i64>, name: &str, no: Option<i32>, date: &str) -> TransferInResult {
    let inp = TransferInInput {
        student_id: id,
        student: input(name, 2026, 3, "나리", no),
        date: date.into(),
    };
    db.write(|c| transfer_in(c, &inp, today())).unwrap()
}

fn go_out(db: &Db, id: i64, date: &str) -> AppResult<()> {
    let inp = TransferOutInput {
        student_id: id,
        school_year: 2026,
        date: date.into(),
        to_school: Some("○○초등학교".into()),
        note: Some("가족 이사".into()),
    };
    db.write(|c| transfer_out(c, &inp, today()))
}

fn status_of(db: &Db, id: i64, year: i32) -> String {
    db.read(|c| {
        Ok(c.query_row(
            "SELECT status FROM enrollments WHERE student_id = ?1 AND school_year = ?2",
            params![id, year],
            |r| r.get(0),
        )?)
    })
    .unwrap()
}

fn enrollment_count(db: &Db, id: i64) -> i64 {
    db.read(|c| {
        Ok(c.query_row(
            "SELECT COUNT(*) FROM enrollments WHERE student_id = ?1",
            [id],
            |r| r.get(0),
        )?)
    })
    .unwrap()
}

fn student_count(db: &Db) -> i64 {
    db.read(|c| Ok(c.query_row("SELECT COUNT(*) FROM students", [], |r| r.get(0))?))
        .unwrap()
}

/// 그 학년도 사건을 일어난 차례대로 `(종류, 날짜)`
fn events(db: &Db, id: i64) -> Vec<(String, Option<String>)> {
    db.read(|c| {
        let mut st = c.prepare(
            "SELECT kind, event_date FROM enrollment_events
              WHERE student_id = ?1 ORDER BY id",
        )?;
        let out: Vec<(String, Option<String>)> = st
            .query_map([id], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(out)
    })
    .unwrap()
}

/// 현재 학생명단(재학생)에 있는가
fn on_roster(db: &Db, id: i64) -> bool {
    db.read(|c| {
        let f = student::ListFilter {
            school_year: 2026,
            ..Default::default()
        };
        let page = student::list(c, &f, 500, 0, today())?;
        Ok(page.rows.iter().any(|r| r.id == id))
    })
    .unwrap()
}

fn moved(db: &Db, want_in: bool) -> Vec<MoveRow> {
    db.read(|c| list(c, 2026, want_in, None, None, today())).unwrap()
}

// ---------------------------------------------------------------
// 신규 전입
// ---------------------------------------------------------------

#[test]
fn 신규_전입생을_등록하면_바로_명단에_나온다() {
    let db = db();
    let out = come_in(&db, None, "새학생가", Some(25), "2026-09-10");

    assert!(out.created, "새 학생을 만들었다");
    assert!(!out.returned);
    assert_eq!(status_of(&db, out.student_id, 2026), "TRANSFER_IN");
    assert!(on_roster(&db, out.student_id), "전입생은 재학생이다");

    assert_eq!(
        events(&db, out.student_id),
        vec![("TRANSFER_IN".to_string(), Some("2026-09-10".to_string()))],
        "첫 사건이 등록이 아니라 전입이다"
    );

    let rows = moved(&db, true);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "새학생가");
    assert_eq!(rows[0].date.as_deref(), Some("2026-09-10"));
    assert_eq!(rows[0].status_label, "전입");
}

#[test]
fn 전입생도_주소_판정과_확인_필요를_그대로_거친다() {
    let db = db();
    let out = come_in(&db, None, "새학생나", Some(26), "2026-09-10");

    // 주소 규칙이 없으니 '주소 확인' 이 열려야 한다 (Phase 3 규칙 그대로)
    let kinds: Vec<String> = db
        .read(|c| issue::list_for_student(c, out.student_id))
        .unwrap()
        .into_iter()
        .map(|i| i.kind)
        .collect();
    assert!(kinds.contains(&"ADDRESS".to_string()), "주소 판정이 돌았다");
}

#[test]
fn 번호가_겹쳐도_전입을_막지_않고_표시만_남긴다() {
    let db = db();
    enroll(&db, "기존학생", 3, "나리", 17);
    let out = come_in(&db, None, "새학생다", Some(17), "2026-09-10");

    assert!(out.number_dup, "겹쳤다는 사실을 알려 준다");
    assert!(on_roster(&db, out.student_id), "그래도 저장은 된다");

    let dup = db
        .read(|c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM issues WHERE kind='NUMBER_DUP' AND status='OPEN'",
                [],
                |r| r.get::<_, i64>(0),
            )?)
        })
        .unwrap();
    assert_eq!(dup, 2, "겹친 두 학생 모두에게 표시한다");
}

// ---------------------------------------------------------------
// 기존 학생 재전입
// ---------------------------------------------------------------

#[test]
fn 같은_학년도에_나갔다_돌아오면_학적을_더_만들지_않는다() {
    let db = db();
    let id = enroll(&db, "돌아온학생", 3, "나리", 7);

    go_out(&db, id, "2026-05-14").unwrap();
    assert!(!on_roster(&db, id), "나가면 명단에서 빠진다");

    let back = come_in(&db, Some(id), "돌아온학생", Some(7), "2026-09-10");
    assert!(!back.created, "학생을 새로 만들지 않는다");
    assert!(back.returned, "돌아온 것으로 본다");
    assert_eq!(back.student_id, id);

    assert_eq!(student_count(&db), 1, "사람은 하나뿐이다");
    assert_eq!(enrollment_count(&db, id), 1, "한 학년도에 학적은 하나다");
    assert!(on_roster(&db, id), "명단으로 돌아온다");

    // 나간 기록이 사라지지 않는다
    assert_eq!(
        events(&db, id),
        vec![
            ("ENROLL".to_string(), None),
            ("TRANSFER_OUT".to_string(), Some("2026-05-14".to_string())),
            ("TRANSFER_IN".to_string(), Some("2026-09-10".to_string())),
        ]
    );
}

#[test]
fn 다른_학년도_재전입은_그_해_학적을_새로_만든다() {
    let db = db();
    let last_year = input("지난해학생", 2025, 2, "나리", Some(7));
    let id = db
        .write(|c| student::create(c, &last_year, today()))
        .unwrap();

    let back = come_in(&db, Some(id), "지난해학생", Some(12), "2026-09-10");
    assert!(!back.created);
    assert_eq!(student_count(&db), 1);
    assert_eq!(enrollment_count(&db, id), 2, "학년도마다 학적이 하나씩");

    // 지난 학년도 학적은 그대로다
    let old: (i32, Option<String>, Option<i32>, String) = db
        .read(|c| {
            Ok(c.query_row(
                "SELECT grade, class_name, class_no, status FROM enrollments
                  WHERE student_id = ?1 AND school_year = 2025",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )?)
        })
        .unwrap();
    assert_eq!(old, (2, Some("나리".into()), Some(7), "ENROLLED".into()));
    assert_eq!(status_of(&db, id, 2026), "TRANSFER_IN");
}

#[test]
fn 이름과_생년월일로_기존_학생을_찾아_준다() {
    let db = db();
    let id = enroll(&db, "찾을학생", 3, "나리", 7);
    go_out(&db, id, "2026-05-14").unwrap();

    let found = db
        .read(|c| search_students(c, "찾을학생", None, 2026))
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].student_id, id);
    assert_eq!(found[0].current_status.as_deref(), Some("전출"));

    let line = &found[0].history[0];
    assert_eq!(line.school_year, 2026);
    assert_eq!(line.where_at, "3-나리 7번", "어디 있었는지 보여 준다");
    assert_eq!(line.extra.as_deref(), Some("2026.05.14. 전출"));

    // 두 글자도 안 되는 검색어는 거절한다 — 온 학교가 걸린다
    assert!(db.read(|c| search_students(c, "찾", None, 2026)).is_err());
}

// ---------------------------------------------------------------
// 전출
// ---------------------------------------------------------------

#[test]
fn 전출하면_명단에서_빠지지만_자료는_남는다() {
    let db = db();
    let id = enroll(&db, "나가는학생", 3, "나리", 7);

    go_out(&db, id, "2026-05-14").unwrap();

    assert_eq!(student_count(&db), 1, "학생을 지우지 않는다");
    assert_eq!(enrollment_count(&db, id), 1, "학적도 지우지 않는다");
    assert_eq!(status_of(&db, id, 2026), "TRANSFER_OUT");
    assert!(!on_roster(&db, id));

    let rows = moved(&db, false);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].date.as_deref(), Some("2026-05-14"));
    assert_eq!(rows[0].to_school.as_deref(), Some("○○초등학교"));
    assert_eq!(rows[0].class_label, "3-나리");
    assert_eq!(rows[0].class_no, Some(7));

    // 사건에 그때의 자리가 함께 남는다
    let snap: (Option<i32>, Option<String>, Option<i32>) = db
        .read(|c| {
            Ok(c.query_row(
                "SELECT grade, class_name, class_no FROM enrollment_events
                  WHERE student_id = ?1 AND kind = 'TRANSFER_OUT'",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?)
        })
        .unwrap();
    assert_eq!(snap, (Some(3), Some("나리".into()), Some(7)));
}

#[test]
fn 전출하면_반별_인원이_그_자리에서_준다() {
    let db = db();
    let a = enroll(&db, "가학생", 3, "나리", 1);
    enroll(&db, "나학생", 3, "나리", 2);
    enroll(&db, "다학생", 3, "나리", 3);

    let before = db.read(|c| stats::grade_counts(c, 2026, 3, today())).unwrap();
    assert_eq!(before.total.counts.total, 3);
    assert_eq!(before.total.counts.male, 3);

    go_out(&db, a, "2026-05-14").unwrap();

    let after = db.read(|c| stats::grade_counts(c, 2026, 3, today())).unwrap();
    assert_eq!(after.total.counts.total, 2);
    assert_eq!(after.total.counts.male, 2);
}

#[test]
fn 이미_나간_학생을_또_전출시키지_않는다() {
    let db = db();
    let id = enroll(&db, "나가는학생", 3, "나리", 7);
    go_out(&db, id, "2026-05-14").unwrap();

    let err = go_out(&db, id, "2026-06-01").unwrap_err();
    assert_eq!(err.code, "INVALID_INPUT");
}

// ---------------------------------------------------------------
// 전출 취소
// ---------------------------------------------------------------

#[test]
fn 전출을_되돌리면_명단으로_돌아오고_기록은_남는다() {
    let db = db();
    let id = enroll(&db, "잘못누른학생", 3, "나리", 7);
    go_out(&db, id, "2026-05-14").unwrap();

    db.write(|c| transfer_out_cancel(c, id, 2026, today()))
        .unwrap();

    assert_eq!(status_of(&db, id, 2026), "ENROLLED");
    assert!(on_roster(&db, id));
    assert!(moved(&db, false).is_empty(), "전출생 화면에서 빠진다");

    // 무슨 일이 있었는지는 남는다
    let kinds: Vec<String> = events(&db, id).into_iter().map(|(k, _)| k).collect();
    assert_eq!(kinds, vec!["ENROLL", "TRANSFER_OUT", "CANCEL"]);

    // 전출 칸도 비워진다
    let out_date: Option<String> = db
        .read(|c| {
            Ok(c.query_row(
                "SELECT transfer_out_date FROM enrollments
                  WHERE student_id = ?1 AND school_year = 2026",
                [id],
                |r| r.get(0),
            )?)
        })
        .unwrap();
    assert_eq!(out_date, None);
}

#[test]
fn 전입생을_되돌리면_전입생으로_돌아온다() {
    let db = db();
    let out = come_in(&db, None, "왔다간학생", Some(25), "2026-09-10");
    go_out(&db, out.student_id, "2026-09-12").unwrap();

    db.write(|c| transfer_out_cancel(c, out.student_id, 2026, today()))
        .unwrap();

    assert_eq!(
        status_of(&db, out.student_id, 2026),
        "TRANSFER_IN",
        "원래 전입생이었으므로 전입생으로 되돌린다"
    );
    assert_eq!(moved(&db, true).len(), 1, "전입생 화면에 다시 나온다");
}

#[test]
fn 나가지_않은_학생은_되돌릴_것이_없다() {
    let db = db();
    let id = enroll(&db, "그냥학생", 3, "나리", 7);
    let err = db
        .write(|c| transfer_out_cancel(c, id, 2026, today()))
        .unwrap_err();
    assert_eq!(err.code, "INVALID_INPUT");
}

// ---------------------------------------------------------------
// 지난 전출생 직접 넣기
// ---------------------------------------------------------------

#[test]
fn 지난_전출생은_명단을_거치지_않고_바로_들어간다() {
    let db = db();
    let inp = PastOutInput {
        student_id: None,
        student: input("지난전출생", 2026, 4, "가람", Some(9)),
        date: "2026-04-20".into(),
        to_school: Some("△△초등학교".into()),
        note: None,
    };
    let id = db
        .write(|c| add_past_transfer_out(c, &inp, today()))
        .unwrap();

    assert!(!on_roster(&db, id), "현재 명단에는 없다");
    assert_eq!(status_of(&db, id, 2026), "TRANSFER_OUT");

    let rows = moved(&db, false);
    assert_eq!(rows.len(), 1, "전출생 화면에는 있다");
    assert_eq!(rows[0].to_school.as_deref(), Some("△△초등학교"));

    assert_eq!(
        events(&db, id),
        vec![("TRANSFER_OUT".to_string(), Some("2026-04-20".to_string()))],
        "등록 사건 없이 전출 사건만 남는다"
    );
}

#[test]
fn 지난_전출생도_이미_있는_학생이면_새로_만들지_않는다() {
    let db = db();
    let last_year = input("옛학생", 2025, 3, "나리", Some(5));
    let id = db
        .write(|c| student::create(c, &last_year, today()))
        .unwrap();

    let inp = PastOutInput {
        student_id: Some(id),
        student: input("옛학생", 2026, 4, "나리", Some(5)),
        date: "2026-04-20".into(),
        to_school: None,
        note: None,
    };
    let same = db
        .write(|c| add_past_transfer_out(c, &inp, today()))
        .unwrap();

    assert_eq!(same, id);
    assert_eq!(student_count(&db), 1, "사람을 두 벌로 만들지 않는다");
    assert_eq!(enrollment_count(&db, id), 2);
}

// ---------------------------------------------------------------
// 날짜
// ---------------------------------------------------------------

#[test]
fn 이력을_깨뜨리는_날짜는_받지_않는다() {
    let db = db();
    let id = enroll(&db, "날짜학생", 3, "나리", 7);

    // 학년도 밖 — 2026학년도는 2026-03-01 부터다
    let inp = TransferOutInput {
        student_id: id,
        school_year: 2026,
        date: "2026-01-05".into(),
        to_school: None,
        note: None,
    };
    let err = db.write(|c| transfer_out(c, &inp, today())).unwrap_err();
    assert!(err.user_message.contains("전출일"));

    // 앞으로도 학년도를 넘기지는 못한다
    let inp = TransferOutInput {
        date: "2027-05-01".into(),
        ..inp
    };
    assert!(db.write(|c| transfer_out(c, &inp, today())).is_err());

    assert_eq!(status_of(&db, id, 2026), "ENROLLED", "하나도 바뀌지 않았다");
}

#[test]
fn 아직_오지_않은_전출일도_받는다() {
    // 학교는 전출 예정일을 미리 알고 며칠 전에 적어 둔다
    let db = db();
    let id = enroll(&db, "예정학생", 3, "나리", 7);

    let inp = TransferOutInput {
        student_id: id,
        school_year: 2026,
        date: "2026-12-01".into(),
        to_school: Some("○○초등학교".into()),
        note: None,
    };
    db.write(|c| transfer_out(c, &inp, today())).unwrap();

    assert_eq!(status_of(&db, id, 2026), "TRANSFER_OUT");
    assert!(on_roster(&db, id), "전출일 전까지는 명단에 그대로 있다");
}

#[test]
fn 전출보다_이른_날짜로_전입시키지_않는다() {
    let db = db();
    let id = enroll(&db, "순서학생", 3, "나리", 7);
    go_out(&db, id, "2026-05-14").unwrap();

    let inp = TransferInInput {
        student_id: Some(id),
        student: input("순서학생", 2026, 3, "나리", Some(7)),
        date: "2026-04-01".into(), // 전출보다 이르다
    };
    let err = db.write(|c| transfer_in(c, &inp, today())).unwrap_err();
    assert!(err.user_message.contains("이릅니다"), "{}", err.user_message);
    assert_eq!(status_of(&db, id, 2026), "TRANSFER_OUT", "그대로 나간 상태다");
}

// ---------------------------------------------------------------
// 두 번 눌러도 안전한가
// ---------------------------------------------------------------

#[test]
fn 같은_전출을_두_번_눌러도_사건이_하나다() {
    let db = db();
    let id = enroll(&db, "두번학생", 3, "나리", 7);
    go_out(&db, id, "2026-05-14").unwrap();

    // 두 번째는 '이미 전출' 로 막히지만, 상태를 억지로 되돌려도 사건은 늘지 않아야 한다
    db.write(|c| {
        Ok(c.execute(
            "UPDATE enrollments SET status = 'ENROLLED' WHERE student_id = ?1",
            [id],
        )?)
    })
    .unwrap();
    go_out(&db, id, "2026-05-14").unwrap();

    let outs = events(&db, id)
        .into_iter()
        .filter(|(k, _)| k == "TRANSFER_OUT")
        .count();
    assert_eq!(outs, 1, "같은 날 같은 종류는 한 번만 적는다");
}

#[test]
fn 취소한_뒤_같은_날로_다시_전출하면_제대로_적힌다() {
    let db = db();
    let id = enroll(&db, "다시학생", 3, "나리", 7);
    go_out(&db, id, "2026-05-14").unwrap();
    db.write(|c| transfer_out_cancel(c, id, 2026, today()))
        .unwrap();
    go_out(&db, id, "2026-05-14").unwrap();

    let kinds: Vec<String> = events(&db, id).into_iter().map(|(k, _)| k).collect();
    assert_eq!(
        kinds,
        vec!["ENROLL", "TRANSFER_OUT", "CANCEL", "TRANSFER_OUT"],
        "취소 뒤의 전출은 새 사건이다"
    );
    assert_eq!(status_of(&db, id, 2026), "TRANSFER_OUT");
}

// ---------------------------------------------------------------
// 형제
// ---------------------------------------------------------------

#[test]
fn 전입생과_기존_학생_사이의_형제_후보를_찾는다() {
    let db = db();
    let mut a = input("형제가", 2026, 5, "가람", Some(3));
    a.father_name = Some("남궁바다".into());
    a.mother_name = Some("제갈하늘".into());
    a.father_phone = Some("010-9000-0001".into());
    a.mother_phone = Some("010-9000-0002".into());
    db.write(|c| student::create(c, &a, today())).unwrap();

    let mut b = input("형제나", 2026, 3, "나리", Some(25));
    b.father_name = Some("남궁바다".into());
    b.mother_name = Some("제갈하늘".into());
    b.father_phone = Some("010-9000-0001".into());
    b.mother_phone = Some("010-9000-0002".into());

    let inp = TransferInInput {
        student_id: None,
        student: b,
        date: "2026-09-10".into(),
    };
    let out = db.write(|c| transfer_in(c, &inp, today())).unwrap();

    assert_eq!(out.sibling_candidates, 1, "들어오자마자 후보를 찾는다");
    let links = db
        .read(|c| Ok(c.query_row("SELECT COUNT(*) FROM sibling_links", [], |r| r.get::<_, i64>(0))?))
        .unwrap();
    assert_eq!(links, 1);
}

#[test]
fn 형제가_전출해도_관계는_남고_본교_형제_수에서만_빠진다() {
    let db = db();
    let mut a = input("남매가", 2026, 5, "가람", Some(3));
    a.father_name = Some("황보구름".into());
    a.father_phone = Some("010-9100-0001".into());
    let a_id = db.write(|c| student::create(c, &a, today())).unwrap();

    let mut b = input("남매나", 2026, 3, "나리", Some(4));
    b.father_name = Some("황보구름".into());
    b.father_phone = Some("010-9100-0001".into());
    let b_id = db.write(|c| student::create(c, &b, today())).unwrap();

    db.write(|c| crate::repo::sibling::scan_for_student(c, 2026, b_id, today()))
        .unwrap();
    let link_id: i64 = db
        .read(|c| Ok(c.query_row("SELECT id FROM sibling_links", [], |r| r.get(0))?))
        .unwrap();
    db.write(|c| crate::repo::sibling::confirm(c, link_id)).unwrap();

    let before = db
        .read(|c| crate::repo::sibling::brief(c, a_id, 2026, today()))
        .unwrap();
    assert_eq!(before.map(|b| b.count), Some(1));

    go_out(&db, b_id, "2026-05-14").unwrap();

    // 관계 자체는 그대로 있다
    let links = db
        .read(|c| Ok(c.query_row("SELECT COUNT(*) FROM sibling_links", [], |r| r.get::<_, i64>(0))?))
        .unwrap();
    assert_eq!(links, 1, "전출했다고 관계를 지우지 않는다");

    // 하지만 '본교 형제' 수에는 들지 않는다
    let after = db
        .read(|c| crate::repo::sibling::brief(c, a_id, 2026, today()))
        .unwrap();
    assert!(after.is_none(), "지금 함께 다니지 않으므로 세지 않는다");

    // 형제 화면에는 왜 빠졌는지 적힌다
    let views = db
        .read(|c| crate::repo::sibling::list_for_student(c, a_id, 2026, today()))
        .unwrap();
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].partner_note.as_deref(), Some("전출"));

    // 돌아오면 다시 센다
    come_in(&db, Some(b_id), "남매나", Some(4), "2026-09-10");
    let back = db
        .read(|c| crate::repo::sibling::brief(c, a_id, 2026, today()))
        .unwrap();
    assert_eq!(back.map(|b| b.count), Some(1), "관계가 손실되지 않았다");
}

// ---------------------------------------------------------------
// 확인 필요
// ---------------------------------------------------------------

#[test]
fn 전출한_학생의_확인_필요는_업무함에서_빠진다() {
    let db = db();
    let id = enroll(&db, "표시학생", 3, "나리", 7);

    let before: i64 = db
        .read(|c| issue::summary(c, 2026, today()))
        .unwrap()
        .iter()
        .map(|k| k.count)
        .sum();
    assert!(before > 0, "주소 미분류 등으로 표시가 있다");

    go_out(&db, id, "2026-05-14").unwrap();

    let after: i64 = db
        .read(|c| issue::summary(c, 2026, today()))
        .unwrap()
        .iter()
        .map(|k| k.count)
        .sum();
    assert_eq!(after, 0, "나간 학생의 일은 오늘 할 일이 아니다");

    // 자료를 지운 것은 아니다 — 학생 상세에서는 그대로 보인다
    let mine = db.read(|c| issue::list_for_student(c, id)).unwrap();
    assert!(!mine.is_empty(), "표시를 지우지는 않는다");

    // 되돌리면 함께 돌아온다
    db.write(|c| transfer_out_cancel(c, id, 2026, today()))
        .unwrap();
    let back: i64 = db
        .read(|c| issue::summary(c, 2026, today()))
        .unwrap()
        .iter()
        .map(|k| k.count)
        .sum();
    assert_eq!(back, before);
}

// ---------------------------------------------------------------
// 예정된 전입·전출 (v0.1.3)
// ---------------------------------------------------------------

/// 기준일을 직접 넣어 '그날의 재학생' 을 센다. 시스템 시계를 보지 않는다.
fn d(m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, m, day).unwrap()
}

/// 기준일에 학생명단(현재 재학생)에 드는가
fn on_roster_at(db: &Db, id: i64, asof: NaiveDate) -> bool {
    db.read(|c| {
        let f = student::ListFilter {
            school_year: 2026,
            ..Default::default()
        };
        let page = student::list(c, &f, 500, 0, asof)?;
        Ok(page.rows.iter().any(|r| r.id == id))
    })
    .unwrap()
}

/// 기준일의 통계 인원
fn counted_at(db: &Db, asof: NaiveDate) -> i64 {
    db.read(|c| stats::totals(c, &stats::StatFilter::year(2026), asof))
        .unwrap()
        .total
}

/// 기준일에 내보내기가 뽑는 인원 — 명단과 같은 조건을 쓰는지 본다
fn exported_at(db: &Db, asof: NaiveDate) -> usize {
    db.read(|c| {
        crate::repo::export::rows(
            c,
            &student::ListFilter {
                school_year: 2026,
                ..Default::default()
            },
            asof,
        )
    })
    .unwrap()
    .len()
}

fn moved_at(db: &Db, want_in: bool, asof: NaiveDate) -> Vec<MoveRow> {
    db.read(|c| list(c, 2026, want_in, None, None, asof)).unwrap()
}

#[test]
fn 미래_전입은_그날이_되어야_명단에_든다() {
    let db = db();
    let inp = TransferInInput {
        student_id: None,
        student: input("예정전입생", 2026, 3, "나리", Some(11)),
        date: "2026-10-05".into(),
    };
    let r = db.write(|c| transfer_in(c, &inp, d(10, 1))).unwrap();
    let id = r.student_id;

    // 전입 관리 화면에는 전입 예정으로 보인다
    let rows = moved_at(&db, true, d(10, 1));
    let row = rows.iter().find(|r| r.student_id == id).unwrap();
    assert_eq!(row.pending.as_deref(), Some("IN"));
    assert_eq!(row.pending_label.as_deref(), Some("전입 예정"));
    assert!(!row.active_now);

    // 10월 1일 — 명단·통계·내보내기 어디에도 없다
    assert!(!on_roster_at(&db, id, d(10, 1)));
    assert_eq!(counted_at(&db, d(10, 1)), 0);
    assert_eq!(exported_at(&db, d(10, 1)), 0);

    // 전입일 당일부터 든다
    assert!(on_roster_at(&db, id, d(10, 5)), "전입일 당일부터 포함한다");
    assert_eq!(counted_at(&db, d(10, 5)), 1);
    assert_eq!(exported_at(&db, d(10, 5)), 1);
    assert!(on_roster_at(&db, id, d(10, 6)));

    // 그날이 되면 더 이상 예정이 아니다
    let rows = moved_at(&db, true, d(10, 5));
    assert_eq!(rows[0].pending, None);
    assert!(rows[0].active_now);
}

#[test]
fn 미래_전출은_그날부터_명단에서_빠진다() {
    let db = db();
    let id = enroll(&db, "예정전출생", 3, "나리", 7);
    let inp = TransferOutInput {
        student_id: id,
        school_year: 2026,
        date: "2026-10-05".into(),
        to_school: Some("○○초등학교".into()),
        note: None,
    };
    db.write(|c| transfer_out(c, &inp, d(10, 1))).unwrap();

    // 전출생 화면에는 전출 예정으로 보인다
    let rows = moved_at(&db, false, d(10, 1));
    assert_eq!(rows[0].pending.as_deref(), Some("OUT"));
    assert_eq!(rows[0].pending_label.as_deref(), Some("전출 예정"));
    assert!(rows[0].active_now, "전출일 전까지는 재학생이다");

    // 10월 1일 — 아직 다닌다
    assert!(on_roster_at(&db, id, d(10, 1)));
    assert_eq!(counted_at(&db, d(10, 1)), 1);
    assert_eq!(exported_at(&db, d(10, 1)), 1);

    // 10월 4일까지 다니고, 5일부터 빠진다
    assert!(on_roster_at(&db, id, d(10, 4)));
    assert!(!on_roster_at(&db, id, d(10, 5)), "전출일 당일부터 제외한다");
    assert_eq!(counted_at(&db, d(10, 5)), 0);
    assert_eq!(exported_at(&db, d(10, 5)), 0);
}

#[test]
fn 예정일을_바꾸면_판정이_바로_따라온다() {
    let db = db();
    let id = enroll(&db, "미루는학생", 3, "나리", 7);
    let inp = TransferOutInput {
        student_id: id,
        school_year: 2026,
        date: "2026-10-05".into(),
        to_school: None,
        note: None,
    };
    db.write(|c| transfer_out(c, &inp, d(10, 1))).unwrap();
    assert!(!on_roster_at(&db, id, d(10, 7)), "10월 5일에 나갔다면 7일에는 없다");

    // 예정일을 10월 10일로 미룬다
    db.write(|c| reschedule(c, id, 2026, "2026-10-10", d(10, 1)))
        .unwrap();

    assert!(
        on_roster_at(&db, id, d(10, 7)),
        "날짜만 바꿨는데 7일에는 다시 재학생이다 (상태를 손으로 고치지 않았다)"
    );
    assert!(!on_roster_at(&db, id, d(10, 10)));

    // 바꾼 사실이 이력에 남는다
    let last = events(&db, id).pop().unwrap();
    assert_eq!(last.0, "TRANSFER_OUT");
    assert_eq!(last.1.as_deref(), Some("2026-10-10"));
}

#[test]
fn 예정일은_앞당길_수도_있다() {
    let db = db();
    let id = enroll(&db, "서두르는학생", 3, "나리", 7);
    let inp = TransferOutInput {
        student_id: id,
        school_year: 2026,
        date: "2026-10-20".into(),
        to_school: None,
        note: None,
    };
    db.write(|c| transfer_out(c, &inp, d(10, 1))).unwrap();

    // 아직 일어나지 않은 이동이므로 스스로를 앞선 이동이라며 막지 않는다
    db.write(|c| reschedule(c, id, 2026, "2026-10-08", d(10, 1)))
        .unwrap();
    assert!(on_roster_at(&db, id, d(10, 7)));
    assert!(!on_roster_at(&db, id, d(10, 8)));
}

#[test]
fn 이미_지난_이동은_예정일_고치기로_바꿀_수_없다() {
    let db = db();
    let id = enroll(&db, "이미간학생", 3, "나리", 7);
    go_out(&db, id, "2026-05-14").unwrap();

    let err = db
        .write(|c| reschedule(c, id, 2026, "2026-06-01", today()))
        .unwrap_err();
    assert_eq!(err.code, "INVALID_INPUT");
    assert!(err.user_message.contains("예정일"));
}

#[test]
fn 전출_예정을_되돌리면_그대로_재학생이다() {
    let db = db();
    let id = enroll(&db, "마음바꾼학생", 3, "나리", 7);
    let inp = TransferOutInput {
        student_id: id,
        school_year: 2026,
        date: "2026-10-05".into(),
        to_school: None,
        note: None,
    };
    db.write(|c| transfer_out(c, &inp, d(10, 1))).unwrap();
    assert!(!on_roster_at(&db, id, d(10, 6)));

    db.write(|c| transfer_out_cancel(c, id, 2026, d(10, 1)))
        .unwrap();

    assert_eq!(status_of(&db, id, 2026), "ENROLLED");
    assert!(on_roster_at(&db, id, d(10, 6)), "되돌리면 날짜와 상관없이 재학생이다");
    assert!(moved_at(&db, false, d(10, 1)).is_empty(), "전출생 화면에서 사라진다");

    let kinds: Vec<String> = events(&db, id).into_iter().map(|(k, _)| k).collect();
    assert!(kinds.iter().any(|k| k == "TRANSFER_OUT"), "나가려 했던 기록은 남는다");
    assert!(kinds.iter().any(|k| k == "CANCEL"));
}

#[test]
fn 새_학생의_전입_예정을_되돌리면_학생_자료까지_사라진다() {
    let db = db();
    let inp = TransferInInput {
        student_id: None,
        student: input("잘못넣은학생", 2026, 3, "나리", Some(11)),
        date: "2026-10-05".into(),
    };
    let id = db.write(|c| transfer_in(c, &inp, d(10, 1))).unwrap().student_id;
    assert_eq!(student_count(&db), 1);

    let out = db
        .write(|c| transfer_in_cancel(c, id, 2026, d(10, 1)))
        .unwrap();

    assert!(out.student_removed, "이번에 처음 만든 학생이므로 함께 지운다");
    assert_eq!(student_count(&db), 0, "유령 학생을 남기지 않는다");
    assert!(moved_at(&db, true, d(10, 1)).is_empty());
}

#[test]
fn 돌아오던_학생의_전입_예정을_되돌리면_학적만_사라진다() {
    let db = db();
    // 지난 학년도에 다녔던 학생
    let i = input("돌아오는학생", 2025, 2, "가람", Some(3));
    let id = db.write(|c| student::create(c, &i, today())).unwrap();

    let inp = TransferInInput {
        student_id: Some(id),
        student: input("돌아오는학생", 2026, 3, "나리", Some(11)),
        date: "2026-10-05".into(),
    };
    db.write(|c| transfer_in(c, &inp, d(10, 1))).unwrap();
    assert_eq!(enrollment_count(&db, id), 2);

    let out = db
        .write(|c| transfer_in_cancel(c, id, 2026, d(10, 1)))
        .unwrap();

    assert!(!out.student_removed, "지난 학적이 있는 학생은 지우지 않는다");
    assert_eq!(enrollment_count(&db, id), 1, "올해 학적만 사라진다");
    assert_eq!(student_count(&db), 1);
}

#[test]
fn 이미_온_학생은_전입_예정_취소로_지울_수_없다() {
    let db = db();
    let r = come_in(&db, None, "이미온학생", Some(11), "2026-09-10");
    let err = db
        .write(|c| transfer_in_cancel(c, r.student_id, 2026, today()))
        .unwrap_err();
    assert_eq!(err.code, "INVALID_INPUT");
    assert!(err.user_message.contains("전출"), "{}", err.user_message);
    assert_eq!(student_count(&db), 1, "학생이 지워지지 않았다");
}

#[test]
fn 전입_예정_학생은_형제_수에도_아직_들지_않는다() {
    let db = db();
    // 보호자가 같은 두 학생 — 한 명은 다니고 한 명은 전입 예정
    let mut here = input("있는형", 2026, 5, "가람", Some(1));
    here.father_name = Some("남궁바다".into());
    here.mother_name = Some("제갈하늘".into());
    let a = db.write(|c| student::create(c, &here, today())).unwrap();

    let mut coming = input("올동생", 2026, 3, "나리", Some(2));
    coming.father_name = Some("남궁바다".into());
    coming.mother_name = Some("제갈하늘".into());
    let inp = TransferInInput {
        student_id: None,
        student: coming,
        date: "2026-10-05".into(),
    };
    db.write(|c| transfer_in(c, &inp, d(10, 1))).unwrap();

    let link = db
        .read(|c| Ok(c.query_row("SELECT id FROM sibling_links", [], |r| r.get(0))?))
        .unwrap();
    db.write(|c| crate::repo::sibling::confirm(c, link)).unwrap();

    let before = db
        .read(|c| crate::repo::sibling::brief(c, a, 2026, d(10, 1)))
        .unwrap();
    assert!(before.is_none(), "아직 오지 않은 형제는 본교 형제에 들지 않는다");

    let after = db
        .read(|c| crate::repo::sibling::brief(c, a, 2026, d(10, 5)))
        .unwrap();
    assert_eq!(after.map(|b| b.count), Some(1), "전입일부터 센다");
}

#[test]
fn 전출_예정_학생은_그날까지_형제로_센다() {
    let db = db();
    let mut big = input("남는형", 2026, 5, "가람", Some(1));
    big.father_name = Some("남궁바다".into());
    big.mother_name = Some("제갈하늘".into());
    let a = db.write(|c| student::create(c, &big, today())).unwrap();

    let mut small = input("떠날동생", 2026, 3, "나리", Some(2));
    small.father_name = Some("남궁바다".into());
    small.mother_name = Some("제갈하늘".into());
    let b = db.write(|c| student::create(c, &small, today())).unwrap();

    db.write(|c| {
        crate::repo::sibling::scan(c, 2026, today(), |_, _, _| {})?;
        let link: i64 = c.query_row("SELECT id FROM sibling_links", [], |r| r.get(0))?;
        crate::repo::sibling::confirm(c, link)?;
        Ok(())
    })
    .unwrap();

    let inp = TransferOutInput {
        student_id: b,
        school_year: 2026,
        date: "2026-10-05".into(),
        to_school: None,
        note: None,
    };
    db.write(|c| transfer_out(c, &inp, d(10, 1))).unwrap();

    let before = db
        .read(|c| crate::repo::sibling::brief(c, a, 2026, d(10, 1)))
        .unwrap();
    assert_eq!(before.map(|x| x.count), Some(1), "아직 함께 다닌다");

    let after = db
        .read(|c| crate::repo::sibling::brief(c, a, 2026, d(10, 5)))
        .unwrap();
    assert!(after.is_none(), "전출일부터 빠진다");

    // 형제 화면은 왜 그런지 알려 준다
    let views = db
        .read(|c| crate::repo::sibling::list_for_student(c, a, 2026, d(10, 1)))
        .unwrap();
    assert_eq!(views[0].partner_note.as_deref(), Some("전출 예정"));
}

#[test]
fn 기존_학생으로_전입하면_주소와_보호자가_그대로_이어진다() {
    let db = db();
    // 지난 학년도에 다녔던 학생 — 주소·보호자·비고가 들어 있다
    let mut before = input("돌아온학생", 2025, 2, "가람", Some(3));
    before.address_raw = Some("○○시 가온로 101, 101동 1001호".into());
    before.father_name = Some("남궁바다".into());
    before.father_phone = Some("010-0000-0002".into());
    before.note = Some("알레르기 있음".into());
    let id = db.write(|c| student::create(c, &before, today())).unwrap();

    // 전입 폼은 기존 학생정보를 그대로 싣고 학적만 새로 정한다
    let mut now = input("돌아온학생", 2026, 3, "나리", Some(11));
    now.address_raw = before.address_raw.clone();
    now.father_name = before.father_name.clone();
    now.father_phone = before.father_phone.clone();
    now.mother_phone = before.mother_phone.clone();
    now.note = before.note.clone();

    let r = db
        .write(|c| {
            transfer_in(
                c,
                &TransferInInput {
                    student_id: Some(id),
                    student: now,
                    date: "2026-09-10".into(),
                },
                today(),
            )
        })
        .unwrap();

    assert_eq!(r.student_id, id, "같은 학생 번호를 그대로 쓴다");
    assert!(!r.created);
    assert_eq!(student_count(&db), 1, "학생을 새로 만들지 않는다");
    assert_eq!(enrollment_count(&db, id), 2, "학년도마다 한 줄");

    let got = db.read(|c| student::detail(c, id, 2026, today())).unwrap();
    assert_eq!(got.note.as_deref(), Some("알레르기 있음"));
    assert_eq!(got.father_name.as_deref(), Some("남궁바다"));
    assert!(got.address_raw.unwrap().contains("가온로 101"));
}
