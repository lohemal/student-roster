//! 학생 저장·조회 검사. 실제 마이그레이션을 적용한 메모리 DB 로 돌린다.
//!
//! 연락처 부분검색처럼 SQL 이 실제로 해 줘야 하는 일은 여기서 확인한다.

use super::*;
use crate::db::Db;
use crate::repo::settings;

fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 9, 15).unwrap()
}

fn db_with_year(year: i32) -> Db {
    let db = Db::memory();
    db.write(|c| {
        settings::create_year(c, year)?;
        settings::set_current_year(c, year)
    })
    .unwrap();
    db
}

/// 시험용 학생 하나. 필요한 항목만 바꿔 쓴다.
fn input(name: &str, grade: i32, class_name: Option<&str>, class_no: Option<i32>) -> StudentInput {
    StudentInput {
        name: name.to_string(),
        gender: Some("M".into()),
        birth_raw: Some("170315".into()),
        address_raw: Some("세종특별자치시 한누리대로 123".into()),
        school_year: 2026,
        grade,
        class_name: class_name.map(str::to_string),
        class_no,
        ..Default::default()
    }
}

fn add(db: &Db, i: &StudentInput) -> i64 {
    db.write(|c| create(c, i, today())).unwrap()
}

fn all(db: &Db, f: &ListFilter) -> Vec<StudentRow> {
    db.read(|c| list(c, f, 1000, 0)).unwrap().rows
}

fn filter() -> ListFilter {
    ListFilter {
        school_year: 2026,
        ..Default::default()
    }
}

fn names(rows: &[StudentRow]) -> Vec<String> {
    rows.iter().map(|r| r.name.clone()).collect()
}

// ---------------------------------------------------------------
// 등록 · 수정
// ---------------------------------------------------------------

#[test]
fn 학생을_등록하면_학적도_함께_생긴다() {
    let db = db_with_year(2026);
    let id = add(&db, &input("홍길동", 3, Some("가람"), Some(7)));

    let d = db.read(|c| detail(c, id, 2026)).unwrap();
    assert_eq!(d.name, "홍길동");
    let e = d.enrollment.expect("2026 학적이 있어야 한다");
    assert_eq!((e.grade, e.class_name.as_deref(), e.class_no), (3, Some("가람"), Some(7)));
    assert_eq!(e.status, "ENROLLED");
    assert_eq!(e.class_label, "3-가람");
}

#[test]
fn 등록하면_학적_이력에_등록_사건이_남는다() {
    let db = db_with_year(2026);
    let id = add(&db, &input("홍길동", 3, Some("가람"), Some(7)));

    let d = db.read(|c| detail(c, id, 2026)).unwrap();
    assert_eq!(d.events.len(), 1);
    assert_eq!(d.events[0].kind, "ENROLL");
    assert_eq!(d.events[0].kind_label, "등록");
    assert_eq!(d.events[0].class_label.as_deref(), Some("3-가람"));
}

#[test]
fn 저장할_때_생년월일과_연락처를_정리한다() {
    let db = db_with_year(2026);
    let mut i = input("홍길동", 1, Some("1"), Some(1));
    i.birth_raw = Some("17.03.15.".into());
    i.father_phone = Some("01012345678".into());
    let id = add(&db, &i);

    let d = db.read(|c| detail(c, id, 2026)).unwrap();
    assert_eq!(d.birth_date.as_deref(), Some("2017-03-15"));
    assert_eq!(d.birth_raw.as_deref(), Some("17.03.15."), "원본은 그대로 둔다");
    assert_eq!(d.father_phone.as_deref(), Some("010-1234-5678"));
}

#[test]
fn 수정하면_기본정보와_학적이_함께_바뀐다() {
    let db = db_with_year(2026);
    let id = add(&db, &input("홍길동", 3, Some("가람"), Some(7)));

    let mut i = input("홍길동", 4, Some("나리"), Some(2));
    i.name = "홍길순".into();
    i.father_phone = Some("010-9999-8888".into());
    db.write(|c| update(c, id, &i, today())).unwrap();

    let d = db.read(|c| detail(c, id, 2026)).unwrap();
    assert_eq!(d.name, "홍길순");
    assert_eq!(d.father_phone.as_deref(), Some("010-9999-8888"));
    let e = d.enrollment.unwrap();
    assert_eq!((e.grade, e.class_name.as_deref(), e.class_no), (4, Some("나리"), Some(2)));
}

#[test]
fn 이름이_없으면_등록하지_않는다() {
    let db = db_with_year(2026);
    let mut i = input("", 1, Some("1"), Some(1));
    i.name = "   ".into();
    let err = db.write(|c| create(c, &i, today())).unwrap_err();
    assert_eq!(err.code, "INVALID_INPUT");
}

#[test]
fn 없는_학년도에는_등록하지_않는다() {
    let db = db_with_year(2026);
    let mut i = input("홍길동", 1, Some("1"), Some(1));
    i.school_year = 2030;
    let err = db.write(|c| create(c, &i, today())).unwrap_err();
    assert_eq!(err.code, "SETUP_REQUIRED");
}

// ---------------------------------------------------------------
// 반 — 숫자 · 한글 · 비어 있음
// ---------------------------------------------------------------

#[test]
fn 반_이름은_숫자든_한글이든_저장된다() {
    let db = db_with_year(2026);
    add(&db, &input("숫자반", 1, Some("1"), Some(1)));
    add(&db, &input("한글반", 1, Some("가람"), Some(1)));

    let rows = all(&db, &filter());
    let labels: Vec<&str> = rows.iter().map(|r| r.class_label.as_str()).collect();
    assert!(labels.contains(&"1-1"));
    assert!(labels.contains(&"1-가람"));
}

#[test]
fn 번호가_비어_있어도_등록된다() {
    let db = db_with_year(2026);
    let id = add(&db, &input("번호없음", 2, Some("나리"), None));

    let d = db.read(|c| detail(c, id, 2026)).unwrap();
    assert_eq!(d.enrollment.unwrap().class_no, None);
    assert_eq!(all(&db, &filter()).len(), 1, "명단에 그대로 나온다");
}

#[test]
fn 반이_비어_있어도_등록된다() {
    let db = db_with_year(2026);
    let id = add(&db, &input("반없음", 2, None, None));

    let d = db.read(|c| detail(c, id, 2026)).unwrap();
    let e = d.enrollment.unwrap();
    assert_eq!(e.class_name, None);
    assert_eq!(e.class_label, "2-미정");
}

#[test]
fn 이름이_같은_학생을_둘_다_등록할_수_있다() {
    let db = db_with_year(2026);
    let mut a = input("김민준", 1, Some("가람"), Some(3));
    a.birth_raw = Some("170301".into());
    let mut b = input("김민준", 2, Some("나리"), Some(5));
    b.birth_raw = Some("160512".into());

    let id_a = add(&db, &a);
    let id_b = add(&db, &b);
    assert_ne!(id_a, id_b);
    assert_eq!(all(&db, &filter()).len(), 2);
}

// ---------------------------------------------------------------
// 정렬
// ---------------------------------------------------------------

#[test]
fn 기본_정렬은_학년_반_번호_이름_순이다() {
    let db = db_with_year(2026);
    add(&db, &input("다솜3번", 2, Some("다솜"), Some(3)));
    add(&db, &input("가람1번", 2, Some("가람"), Some(1)));
    add(&db, &input("가람2번", 2, Some("가람"), Some(2)));
    add(&db, &input("일학년", 1, Some("나리"), Some(9)));

    assert_eq!(
        names(&all(&db, &filter())),
        vec!["일학년", "가람1번", "가람2번", "다솜3번"]
    );
}

#[test]
fn 숫자_반이_글자_반보다_먼저_나오고_숫자_순서를_지킨다() {
    let db = db_with_year(2026);
    add(&db, &input("가람", 1, Some("가람"), Some(1)));
    add(&db, &input("십반", 1, Some("10"), Some(1)));
    add(&db, &input("이반", 1, Some("2"), Some(1)));

    assert_eq!(names(&all(&db, &filter())), vec!["이반", "십반", "가람"]);
}

#[test]
fn 번호가_없는_학생은_반의_맨_뒤에_온다() {
    let db = db_with_year(2026);
    add(&db, &input("번호없음", 1, Some("가람"), None));
    add(&db, &input("오번", 1, Some("가람"), Some(5)));

    assert_eq!(names(&all(&db, &filter())), vec!["오번", "번호없음"]);
}

// ---------------------------------------------------------------
// 연락처 검색 — 요구사항의 핵심
// ---------------------------------------------------------------

fn db_with_phones() -> Db {
    let db = db_with_year(2026);
    let mut a = input("홍길동", 3, Some("가람"), Some(1));
    a.father_phone = Some("010-1234-5678".into());
    a.mother_phone = Some("010-2222-3333".into());
    a.primary_phone = Some("010-1234-5678".into());
    add(&db, &a);

    let mut b = input("김영희", 3, Some("가람"), Some(2));
    b.father_phone = Some("010-9999-0000".into());
    b.primary_phone = Some("010-8888-7777".into());
    add(&db, &b);
    db
}

#[test]
fn 연락처는_가운데_네자리로도_끝_네자리로도_찾힌다() {
    let db = db_with_phones();
    for term in ["1234", "5678", "01012345678", "010-1234-5678", "12345678"] {
        let f = ListFilter {
            q: Some(term.into()),
            ..filter()
        };
        assert_eq!(names(&all(&db, &f)), vec!["홍길동"], "검색어 {term}");
    }
}

#[test]
fn 항목별_연락처_검색은_그_항목만_본다() {
    let db = db_with_phones();

    let f = ListFilter {
        father_phone: Some("9999".into()),
        ..filter()
    };
    assert_eq!(names(&all(&db, &f)), vec!["김영희"]);

    // 같은 숫자를 어머니 연락처 칸에서 찾으면 아무도 없다
    let f = ListFilter {
        mother_phone: Some("9999".into()),
        ..filter()
    };
    assert!(all(&db, &f).is_empty());
}

#[test]
fn 주보호자_연락처로도_찾는다() {
    let db = db_with_phones();
    let f = ListFilter {
        primary_phone: Some("8888".into()),
        ..filter()
    };
    assert_eq!(names(&all(&db, &f)), vec!["김영희"]);
}

#[test]
fn 너무_짧은_연락처_검색어는_온_학교를_불러오지_않는다() {
    let db = db_with_phones();
    let f = ListFilter {
        father_phone: Some("1".into()),
        ..filter()
    };
    assert!(all(&db, &f).is_empty(), "한 글자로 전체가 걸리면 안 된다");
}

#[test]
fn 통합검색에서_한두자리_숫자는_출석번호로_본다() {
    let db = db_with_phones();
    let f = ListFilter {
        q: Some("2".into()),
        ..filter()
    };
    assert_eq!(names(&all(&db, &f)), vec!["김영희"], "2번 학생");
}

#[test]
fn 숫자만_친_검색어는_주소에_섞인_숫자를_끌어오지_않는다() {
    let db = db_with_year(2026);
    let mut a = input("주소에123", 1, Some("가람"), Some(9));
    a.address_raw = Some("세종특별자치시 한누리대로 123".into());
    a.primary_phone = Some("010-5555-6666".into());
    add(&db, &a);
    let mut b = input("이번학생", 1, Some("가람"), Some(2));
    b.address_raw = Some("세종특별자치시 도움로 77".into());
    add(&db, &b);

    // '2' 는 출석번호로만 본다. 주소의 '123' 이 걸리면 안 된다.
    let f = ListFilter {
        q: Some("2".into()),
        ..filter()
    };
    assert_eq!(names(&all(&db, &f)), vec!["이번학생"]);

    // '5555' 는 연락처로만 본다
    let f = ListFilter {
        q: Some("5555".into()),
        ..filter()
    };
    assert_eq!(names(&all(&db, &f)), vec!["주소에123"]);
}

#[test]
fn 통합검색은_이름과_주소도_함께_본다() {
    let db = db_with_year(2026);
    let mut a = input("홍길동", 1, Some("가람"), Some(1));
    a.address_raw = Some("세종특별자치시 한누리대로 123".into());
    add(&db, &a);
    let mut b = input("김영희", 1, Some("가람"), Some(2));
    b.address_raw = Some("세종특별자치시 도움3로 55".into());
    add(&db, &b);

    let by_name = ListFilter {
        q: Some("홍길".into()),
        ..filter()
    };
    assert_eq!(names(&all(&db, &by_name)), vec!["홍길동"]);

    let by_addr = ListFilter {
        q: Some("도움3로".into()),
        ..filter()
    };
    assert_eq!(names(&all(&db, &by_addr)), vec!["김영희"]);
}

// ---------------------------------------------------------------
// 필터
// ---------------------------------------------------------------

#[test]
fn 학년과_반으로_추린다() {
    let db = db_with_year(2026);
    add(&db, &input("삼가람", 3, Some("가람"), Some(1)));
    add(&db, &input("삼나리", 3, Some("나리"), Some(1)));
    add(&db, &input("사가람", 4, Some("가람"), Some(1)));

    let f = ListFilter {
        grade: Some(3),
        ..filter()
    };
    assert_eq!(all(&db, &f).len(), 2);

    let f = ListFilter {
        grade: Some(3),
        class_name: Some("가람".into()),
        ..filter()
    };
    assert_eq!(names(&all(&db, &f)), vec!["삼가람"]);
}

#[test]
fn 쪽_나누기와_전체_수가_맞는다() {
    let db = db_with_year(2026);
    for n in 1..=25 {
        add(&db, &input(&format!("학생{n:02}"), 1, Some("가람"), Some(n)));
    }

    let page = db.read(|c| list(c, &filter(), 10, 0)).unwrap();
    assert_eq!(page.total, 25, "전체 수는 쪽과 무관하다");
    assert_eq!(page.rows.len(), 10);
    assert_eq!(page.rows[0].name, "학생01");

    let page3 = db.read(|c| list(c, &filter(), 10, 20)).unwrap();
    assert_eq!(page3.rows.len(), 5);
    assert_eq!(page3.rows[0].name, "학생21");
}

#[test]
fn 학년도가_다르면_명단에_나오지_않는다() {
    let db = db_with_year(2026);
    db.write(|c| settings::create_year(c, 2027)).unwrap();

    add(&db, &input("올해학생", 1, Some("가람"), Some(1)));
    let mut next = input("내년학생", 1, Some("가람"), Some(1));
    next.school_year = 2027;
    add(&db, &next);

    assert_eq!(names(&all(&db, &filter())), vec!["올해학생"]);
    let f2027 = ListFilter {
        school_year: 2027,
        ..filter()
    };
    assert_eq!(names(&all(&db, &f2027)), vec!["내년학생"]);
}

#[test]
fn 전출한_학생은_기본_명단에서_빠지고_전체보기에는_나온다() {
    let db = db_with_year(2026);
    let id = add(&db, &input("전출예정", 1, Some("가람"), Some(1)));
    add(&db, &input("재학중", 1, Some("가람"), Some(2)));

    db.write(|c| {
        c.execute(
            "UPDATE enrollments SET status = 'TRANSFER_OUT' WHERE student_id = ?1",
            [id],
        )?;
        Ok(())
    })
    .unwrap();

    assert_eq!(names(&all(&db, &filter())), vec!["재학중"]);

    let f = ListFilter {
        status: Some("ALL".into()),
        ..filter()
    };
    assert_eq!(all(&db, &f).len(), 2);

    let f = ListFilter {
        status: Some("TRANSFER_OUT".into()),
        ..filter()
    };
    assert_eq!(names(&all(&db, &f)), vec!["전출예정"]);
}

// ---------------------------------------------------------------
// 여러 학년도 — 과거 기록을 덮어쓰지 않는다
// ---------------------------------------------------------------

#[test]
fn 학년도가_바뀌어도_지난_학적이_그대로_남는다() {
    let db = db_with_year(2026);
    db.write(|c| settings::create_year(c, 2027)).unwrap();

    let id = add(&db, &input("홍길동", 3, Some("가람"), Some(7)));

    // 다음 학년도 학적을 새로 만든다 (Phase 9 의 진급이 하게 될 일)
    db.write(|c| {
        c.execute(
            "INSERT INTO enrollments(student_id, school_year, grade, class_name, class_no, status)
             VALUES (?1, 2027, 4, '나리', 3, 'ENROLLED')",
            [id],
        )?;
        Ok(())
    })
    .unwrap();

    let d = db.read(|c| detail(c, id, 2027)).unwrap();
    assert_eq!(d.enrollments.len(), 2);
    assert_eq!(d.enrollments[0].school_year, 2027, "최근 학년도가 먼저");
    assert_eq!(d.enrollments[0].class_label, "4-나리");
    assert_eq!(d.enrollments[1].class_label, "3-가람", "지난 기록 그대로");

    // 2026 학년도 명단은 여전히 3-가람 7번이다
    let rows = all(&db, &filter());
    assert_eq!(rows[0].class_label, "3-가람");
    assert_eq!(rows[0].class_no, Some(7));
}

#[test]
fn 한_학년도에_학적은_하나만_생긴다() {
    let db = db_with_year(2026);
    let id = add(&db, &input("홍길동", 3, Some("가람"), Some(7)));

    let err = db.write(|c| {
        c.execute(
            "INSERT INTO enrollments(student_id, school_year, grade, class_name, class_no, status)
             VALUES (?1, 2026, 4, '나리', 3, 'ENROLLED')",
            [id],
        )?;
        Ok(())
    });
    assert!(err.is_err(), "같은 학년도에 두 번째 학적이 들어가면 안 된다");
}

// ---------------------------------------------------------------
// 확인 필요
// ---------------------------------------------------------------

#[test]
fn 생년월일이_이상해도_등록은_되고_확인_필요가_붙는다() {
    let db = db_with_year(2026);
    let mut i = input("홍길동", 1, Some("가람"), Some(1));
    i.birth_raw = Some("20170230".into()); // 달력에 없는 날
    let id = add(&db, &i);

    let d = db.read(|c| detail(c, id, 2026)).unwrap();
    assert_eq!(d.birth_date, None);
    assert_eq!(d.birth_raw.as_deref(), Some("20170230"), "원본은 남는다");
    assert!(
        d.issues.iter().any(|i| i.kind == "BIRTH"),
        "생년월일 확인 필요가 있어야 한다"
    );
}

#[test]
fn 자료를_고치면_확인_필요가_저절로_닫힌다() {
    let db = db_with_year(2026);
    let mut i = input("홍길동", 1, Some("가람"), Some(1));
    i.birth_raw = Some("20170230".into());
    let id = add(&db, &i);
    assert!(db
        .read(|c| detail(c, id, 2026))
        .unwrap()
        .issues
        .iter()
        .any(|i| i.kind == "BIRTH"));

    i.birth_raw = Some("20170315".into());
    db.write(|c| update(c, id, &i, today())).unwrap();

    let d = db.read(|c| detail(c, id, 2026)).unwrap();
    assert_eq!(d.birth_date.as_deref(), Some("2017-03-15"));
    assert!(
        !d.issues.iter().any(|i| i.kind == "BIRTH"),
        "고쳤으면 표시가 사라져야 한다"
    );
}

#[test]
fn 반이나_번호가_비면_확인_필요가_붙는다() {
    let db = db_with_year(2026);
    let id = add(&db, &input("반없음", 1, None, None));

    let d = db.read(|c| detail(c, id, 2026)).unwrap();
    let issue = d
        .issues
        .iter()
        .find(|i| i.kind == "CLASS_ASSIGN")
        .expect("반·번호 미정 표시");
    assert!(issue.message.contains("반"));
    assert!(issue.message.contains("번호"));
}

#[test]
fn 비어_있는_항목을_알려준다() {
    let db = db_with_year(2026);
    let mut i = input("빈칸많음", 1, Some("가람"), Some(1));
    i.gender = None;
    i.address_raw = None;
    let id = add(&db, &i);

    let d = db.read(|c| detail(c, id, 2026)).unwrap();
    let issue = d
        .issues
        .iter()
        .find(|i| i.kind == "MISSING")
        .expect("필수 정보 누락 표시");
    assert!(issue.message.contains("성별"), "{}", issue.message);
    assert!(issue.message.contains("주소"), "{}", issue.message);
    assert!(
        issue.message.contains("보호자 연락처"),
        "{}",
        issue.message
    );
}

#[test]
fn 같은_학년도에_이름과_생년월일이_같으면_중복_의심이_붙는다() {
    let db = db_with_year(2026);
    let mut a = input("김민준", 1, Some("가람"), Some(1));
    a.birth_raw = Some("20170315".into());
    let mut b = input("김민준", 2, Some("나리"), Some(1));
    b.birth_raw = Some("20170315".into());

    add(&db, &a);
    let id_b = add(&db, &b);

    let d = db.read(|c| detail(c, id_b, 2026)).unwrap();
    assert!(d.issues.iter().any(|i| i.kind == "DUPLICATE"));
}

#[test]
fn 이름만_같고_생일이_다르면_중복_의심이_아니다() {
    let db = db_with_year(2026);
    let mut a = input("김민준", 1, Some("가람"), Some(1));
    a.birth_raw = Some("20170315".into());
    let mut b = input("김민준", 2, Some("나리"), Some(1));
    b.birth_raw = Some("20160512".into());

    add(&db, &a);
    let id_b = add(&db, &b);

    let d = db.read(|c| detail(c, id_b, 2026)).unwrap();
    assert!(!d.issues.iter().any(|i| i.kind == "DUPLICATE"));
}

#[test]
fn 같은_확인_필요가_두_번_쌓이지_않는다() {
    let db = db_with_year(2026);
    let mut i = input("홍길동", 1, None, None);
    i.gender = None;
    let id = add(&db, &i);

    // 여러 번 저장해도 표시는 종류마다 하나씩이다
    for _ in 0..3 {
        db.write(|c| update(c, id, &i, today())).unwrap();
    }

    let d = db.read(|c| detail(c, id, 2026)).unwrap();
    let missing = d.issues.iter().filter(|i| i.kind == "MISSING").count();
    let class = d.issues.iter().filter(|i| i.kind == "CLASS_ASSIGN").count();
    assert_eq!((missing, class), (1, 1));
}

#[test]
fn 확인_필요만_모아_볼_수_있다() {
    let db = db_with_year(2026);
    let mut good = input("정상", 1, Some("가람"), Some(1));
    good.primary_phone = Some("010-1111-2222".into()); // 빠진 항목이 없어야 표시가 안 붙는다
    add(&db, &good);
    let mut bad = input("문제있음", 1, None, None);
    bad.gender = None;
    add(&db, &bad);

    let f = ListFilter {
        only_issues: true,
        ..filter()
    };
    assert_eq!(names(&all(&db, &f)), vec!["문제있음"]);
}

#[test]
fn 확인_필요_집계는_올해_학생만_센다() {
    let db = db_with_year(2026);
    db.write(|c| settings::create_year(c, 2027)).unwrap();

    let mut bad = input("올해문제", 1, None, None);
    bad.gender = None;
    add(&db, &bad);

    let counts = db.read(|c| issue::summary(c, 2026)).unwrap();
    assert!(counts.iter().any(|x| x.kind == "CLASS_ASSIGN" && x.count == 1));

    let next = db.read(|c| issue::summary(c, 2027)).unwrap();
    assert!(next.is_empty(), "내년 학년도에는 아직 학생이 없다");
}

// ---------------------------------------------------------------
// 삭제 · 반 목록
// ---------------------------------------------------------------

#[test]
fn 입력_실수를_지울_수_있다() {
    let db = db_with_year(2026);
    let id = add(&db, &input("잘못입력", 1, Some("가람"), Some(1)));

    db.write(|c| delete(c, id)).unwrap();
    assert!(all(&db, &filter()).is_empty());
    assert!(db.read(|c| detail(c, id, 2026)).is_err());
}

#[test]
fn 졸업_기록이_있는_학생은_지우지_못한다() {
    let db = db_with_year(2026);
    let id = add(&db, &input("졸업생", 6, Some("가람"), Some(1)));
    db.write(|c| {
        c.execute(
            "INSERT INTO graduations(student_id, school_year) VALUES (?1, 2026)",
            [id],
        )?;
        Ok(())
    })
    .unwrap();

    let err = db.write(|c| delete(c, id)).unwrap_err();
    assert_eq!(err.code, "IN_USE");
}

#[test]
fn 지우면_딸린_확인_필요도_함께_사라진다() {
    let db = db_with_year(2026);
    let mut bad = input("문제있음", 1, None, None);
    bad.gender = None;
    let id = add(&db, &bad);

    db.write(|c| delete(c, id)).unwrap();
    let left: i64 = db
        .read(|c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM issues WHERE student_id = ?1",
                [id],
                |r| r.get(0),
            )?)
        })
        .unwrap();
    assert_eq!(left, 0);
}

#[test]
fn 실제로_있는_반만_선택지로_준다() {
    let db = db_with_year(2026);
    add(&db, &input("가", 1, Some("나리"), Some(1)));
    add(&db, &input("나", 1, Some("가람"), Some(2)));
    add(&db, &input("다", 2, Some("2"), Some(1)));
    add(&db, &input("라", 2, Some("10"), Some(1)));
    add(&db, &input("마", 3, None, None));

    let opts = db.read(|c| class_options(c, 2026)).unwrap();
    let got: Vec<(i32, &str)> = opts
        .iter()
        .map(|o| (o.grade, o.class_name.as_str()))
        .collect();
    assert_eq!(
        got,
        vec![(1, "가람"), (1, "나리"), (2, "2"), (2, "10")],
        "학년 순 → 숫자 반 → 가나다. 반 미정은 선택지에 없다"
    );
    assert_eq!(opts[0].count, 1);
}

#[test]
fn 졸업한_해의_명단에는_졸업_표시가_붙는다() {
    let db = db_with_year(2026);
    let id = add(&db, &input("졸업생", 6, Some("가람"), Some(1)));
    db.write(|c| {
        c.execute(
            "INSERT INTO graduations(student_id, school_year) VALUES (?1, 2026)",
            [id],
        )?;
        Ok(())
    })
    .unwrap();

    let rows = all(&db, &filter());
    assert!(rows[0].graduated, "그 해 졸업한 학생은 표시가 달라야 한다");
    assert_eq!(rows[0].status, "ENROLLED", "학적 자체는 그대로 재학이다");
}
