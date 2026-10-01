//! 인원 집계 검사.
//!
//! 여기서 보는 것 하나: **모든 표가 같은 숫자를 말하는가.**
//! 남 + 여 + 미입력 = 학년 합계 = 반 합계 = 주소 합계 = 전체.
//! 하나라도 어긋나면 사용자는 어느 숫자를 믿어야 할지 알 수 없다. 이름은 모두 가상이다.

use super::*;
use crate::db::Db;
use crate::repo::{address as addr_repo, settings, student, transfer};
use chrono::NaiveDate;

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

/// 학생 하나. `gender` 는 "M" / "F" / "" (미입력), `address` 는 ""면 주소 없음.
#[allow(clippy::too_many_arguments)]
fn add_at(
    db: &Db,
    year: i32,
    name: &str,
    grade: i32,
    class_name: Option<&str>,
    no: Option<i32>,
    gender: &str,
    address: &str,
) -> i64 {
    let input = student::StudentInput {
        name: name.into(),
        gender: (!gender.is_empty()).then(|| gender.to_string()),
        birth_raw: Some("170315".into()),
        address_raw: (!address.is_empty()).then(|| address.to_string()),
        school_year: year,
        grade,
        class_name: class_name.map(str::to_string),
        class_no: no,
        ..Default::default()
    };
    db.write(|c| student::create(c, &input, today())).unwrap()
}

fn add(db: &Db, name: &str, grade: i32, class_name: &str, no: i32, gender: &str) -> i64 {
    add_at(
        db,
        2026,
        name,
        grade,
        Some(class_name),
        Some(no),
        gender,
        "○○시 가온로 101",
    )
}

fn year(y: i32) -> StatFilter {
    StatFilter::year(y)
}

fn counts(db: &Db, grade: i32) -> Vec<ClassCount> {
    db.read(|c| {
        by_class(
            c,
            &StatFilter {
                grade: Some(grade),
                ..year(2026)
            },
            today(),
        )
    })
    .unwrap()
}

fn find<'a>(rows: &'a [ClassCount], label: &str) -> &'a ClassCount {
    rows.iter()
        .find(|r| r.class_label == label)
        .unwrap_or_else(|| panic!("{label} 반이 없다"))
}

fn leave(db: &Db, student_id: i64) {
    let input = transfer::TransferOutInput {
        student_id,
        school_year: 2026,
        // 전입 뒤에 나가는 경우도 있으므로 늦은 날짜를 쓴다 (이동 순서가 뒤집히면 거절된다)
        date: "2026-09-12".into(),
        to_school: None,
        note: None,
    };
    db.write(|c| transfer::transfer_out(c, &input, today()))
        .unwrap()
}

/// 모든 표가 같은 숫자를 말하는지 한 번에 본다.
fn assert_consistent(db: &Db, f: &StatFilter) -> Consistency {
    let ck = db.read(|c| check_consistency(c, f, today())).unwrap();
    assert!(
        ck.ok,
        "표마다 숫자가 다르다: 전체 {} · 성별 {} · 학년 {} · 반 {} · 주소 {} · 교차 {}",
        ck.total, ck.gender_sum, ck.grade_sum, ck.class_sum, ck.address_sum, ck.cross_sum
    );
    ck
}

// ---------------------------------------------------------------
// 반별 (Phase 6 부터 있던 것)
// ---------------------------------------------------------------

#[test]
fn 반별로_남녀와_미입력을_따로_센다() {
    let db = db();
    for i in 1..=12 {
        add(&db, &format!("일반가{i:02}"), 3, "1", i, "M");
    }
    for i in 13..=23 {
        add(&db, &format!("일반나{i:02}"), 3, "1", i, "F");
    }
    add(&db, "일반다24", 3, "1", 24, ""); // 성별 미입력

    for i in 1..=11 {
        add(&db, &format!("이반가{i:02}"), 3, "2", i, "M");
    }
    for i in 12..=23 {
        add(&db, &format!("이반나{i:02}"), 3, "2", i, "F");
    }

    let rows = counts(&db, 3);
    let one = find(&rows, "3-1").counts.clone();
    assert_eq!((one.male, one.female, one.unknown, one.total), (12, 11, 1, 24));

    let two = find(&rows, "3-2").counts.clone();
    assert_eq!((two.male, two.female, two.unknown, two.total), (11, 12, 0, 23));
}

#[test]
fn 남더하기_여더하기_미입력이_합계와_같다() {
    let db = db();
    add(&db, "가학생", 4, "가람", 1, "M");
    add(&db, "나학생", 4, "가람", 2, "F");
    add(&db, "다학생", 4, "가람", 3, "");
    add(&db, "라학생", 4, "가람", 4, "");

    for r in counts(&db, 4) {
        assert!(
            r.counts.balanced(),
            "{} 반의 숫자가 서로 맞지 않는다",
            r.class_label
        );
    }
}

#[test]
fn 성별이_비어_있는_학생을_남이나_여에_넣지_않는다() {
    let db = db();
    add(&db, "가학생", 5, "가람", 1, "");
    add(&db, "나학생", 5, "가람", 2, "");

    let rows = counts(&db, 5);
    let one = &find(&rows, "5-가람").counts;
    assert_eq!((one.male, one.female), (0, 0), "어느 쪽에도 넣지 않는다");
    assert_eq!(one.unknown, 2);
    assert_eq!(one.total, 2);
}

#[test]
fn 전출한_학생은_그_즉시_빠진다() {
    let db = db();
    let a = add(&db, "가학생", 3, "나리", 1, "M");
    add(&db, "나학생", 3, "나리", 2, "M");
    add(&db, "다학생", 3, "나리", 3, "F");

    let before = counts(&db, 3);
    assert_eq!(find(&before, "3-나리").counts.total, 3);
    assert_eq!(find(&before, "3-나리").counts.male, 2);

    leave(&db, a);

    let after = counts(&db, 3);
    assert_eq!(find(&after, "3-나리").counts.male, 1, "남학생이 하나 줄었다");
    assert_eq!(find(&after, "3-나리").counts.female, 1);
    assert_eq!(find(&after, "3-나리").counts.total, 2);
}

#[test]
fn 다른_학년은_섞이지_않는다() {
    let db = db();
    add(&db, "가학생", 3, "가람", 1, "M");
    add(&db, "나학생", 4, "가람", 1, "M");
    add(&db, "다학생", 5, "가람", 1, "F");

    let three = counts(&db, 3);
    assert_eq!(three.len(), 1);
    assert_eq!(three[0].counts.total, 1);
    assert_eq!(three[0].grade, 3);

    let all = db.read(|c| by_class(c, &year(2026), today())).unwrap();
    assert_eq!(all.len(), 3);
    assert_eq!(all.iter().map(|r| r.counts.total).sum::<i64>(), 3);
}

#[test]
fn 반이_숫자면_숫자_차례로_나온다() {
    let db = db();
    for (i, name) in ["1", "2", "3", "10"].iter().enumerate() {
        add(&db, &format!("학생{i}"), 6, name, 1, "M");
    }
    let labels: Vec<String> = counts(&db, 6).into_iter().map(|r| r.class_label).collect();
    assert_eq!(labels, vec!["6-1", "6-2", "6-3", "6-10"]);
}

#[test]
fn 숫자반과_글자반이_섞여도_명단과_같은_차례다() {
    let db = db();
    for (i, name) in ["나리", "2", "가람", "1"].iter().enumerate() {
        add(&db, &format!("학생{i}"), 6, name, 1, "M");
    }
    let labels: Vec<String> = counts(&db, 6).into_iter().map(|r| r.class_label).collect();
    assert_eq!(
        labels,
        vec!["6-1", "6-2", "6-가람", "6-나리"],
        "숫자 반 먼저, 그다음 글자 반을 가나다 순으로"
    );
}

#[test]
fn 학년_합계도_함께_준다() {
    let db = db();
    add(&db, "가학생", 3, "가람", 1, "M");
    add(&db, "나학생", 3, "가람", 2, "F");
    add(&db, "다학생", 3, "나리", 1, "M");
    add(&db, "라학생", 3, "나리", 2, "");

    let g = db.read(|c| grade_counts(c, 2026, 3, today())).unwrap();
    assert_eq!(g.classes.len(), 2);
    let t = &g.total.counts;
    assert_eq!((t.male, t.female, t.unknown, t.total), (2, 1, 1, 4));
    assert!(g.total.class_label.contains("합계"));
}

#[test]
fn 그_반에서_쓰이는_번호를_알려_준다() {
    let db = db();
    add(&db, "가학생", 3, "나리", 1, "M");
    add(&db, "나학생", 3, "나리", 7, "F");
    let out = add(&db, "다학생", 3, "나리", 9, "M");

    let used = db.read(|c| used_numbers(c, 2026, 3, Some("나리"), today())).unwrap();
    assert_eq!(used, vec![1, 7, 9]);

    leave(&db, out);
    let used = db.read(|c| used_numbers(c, 2026, 3, Some("나리"), today())).unwrap();
    assert_eq!(used, vec![1, 7]);
}

// ---------------------------------------------------------------
// 빠뜨리면 안 되는 학생
// ---------------------------------------------------------------

#[test]
fn 반이_없는_학생도_미정으로_센다() {
    let db = db();
    add(&db, "가학생", 3, "나리", 1, "M");
    add_at(&db, 2026, "나학생", 3, None, Some(2), "F", "○○시 가온로 101");
    add_at(&db, 2026, "다학생", 3, Some(""), None, "M", "○○시 가온로 101");

    let rows = counts(&db, 3);
    let undecided = find(&rows, "3-미정");
    assert_eq!(undecided.counts.total, 2, "반 이름이 비어 있어도 함께 센다");

    // 빠뜨리면 반 합계가 전체와 달라진다
    assert_consistent(&db, &year(2026));
}

#[test]
fn 번호가_없어도_현재_재학생이면_센다() {
    let db = db();
    add_at(&db, 2026, "가학생", 3, Some("나리"), None, "M", "○○시 가온로 101");
    add_at(&db, 2026, "나학생", 3, Some("나리"), Some(2), "F", "○○시 가온로 101");

    assert_eq!(find(&counts(&db, 3), "3-나리").counts.total, 2);
    assert_consistent(&db, &year(2026));
}

// ---------------------------------------------------------------
// 전체 · 학년별
// ---------------------------------------------------------------

#[test]
fn 전체는_남녀와_미입력의_합이다() {
    let db = db();
    add(&db, "가학생", 1, "가람", 1, "M");
    add(&db, "나학생", 1, "가람", 2, "M");
    add(&db, "다학생", 2, "나리", 1, "F");
    add(&db, "라학생", 3, "다솜", 1, "");

    let t = db.read(|c| totals(c, &year(2026), today())).unwrap();
    assert_eq!((t.male, t.female, t.unknown, t.total), (2, 1, 1, 4));
    assert!(t.balanced());
}

#[test]
fn 학년별_합계의_총합이_전체와_같다() {
    let db = db();
    for g in 1..=6 {
        for i in 1..=(g * 2) {
            add(&db, &format!("학생{g}{i:02}"), g, "가람", i, if i % 2 == 0 { "M" } else { "F" });
        }
    }
    let rows = db.read(|c| by_grade(c, &year(2026), today())).unwrap();
    assert_eq!(rows.len(), 6, "학생이 있는 학년만 나온다");
    assert_eq!(rows.iter().map(|r| r.grade).collect::<Vec<_>>(), vec![1, 2, 3, 4, 5, 6]);

    let ck = assert_consistent(&db, &year(2026));
    assert_eq!(ck.total, (1..=6).map(|g| g * 2).sum::<i64>());
}

#[test]
fn 학생이_없는_학년은_표에_나오지_않는다() {
    let db = db();
    add(&db, "가학생", 2, "가람", 1, "M");
    add(&db, "나학생", 5, "가람", 1, "F");

    let rows = db.read(|c| by_grade(c, &year(2026), today())).unwrap();
    assert_eq!(rows.iter().map(|r| r.grade).collect::<Vec<_>>(), vec![2, 5]);
    assert_eq!(
        db.read(|c| grades_present(c, &year(2026), today())).unwrap(),
        vec![2, 5]
    );
}

// ---------------------------------------------------------------
// 주소
// ---------------------------------------------------------------

/// 분류 하나를 만들고 그 이름이 든 주소에 걸리는 규칙을 둔다.
fn make_category(db: &Db, name: &str) -> i64 {
    let name = name.to_string();
    db.write(|c| addr_repo::create_category(c, &name)).unwrap()
}

fn set_manual(db: &Db, student_id: i64, category_id: i64) {
    db.write(|c| addr_repo::set_manual(c, student_id, Some(category_id))).unwrap();
}

#[test]
fn 미분류와_기타와_주소_없음을_섞지_않는다() {
    let db = db();
    let other = db
        .read(|c| {
            Ok(c.query_row(
                "SELECT id FROM address_categories WHERE name = '기타'",
                [],
                |r| r.get::<_, i64>(0),
            )?)
        })
        .unwrap();

    // 사람이 '기타' 라고 정한 학생
    let a = add_at(&db, 2026, "기타학생", 1, Some("가람"), Some(1), "M", "○○시 사온로 707");
    set_manual(&db, a, other);
    // 주소는 있는데 아직 분류하지 못한 학생
    add_at(&db, 2026, "미분류학생", 1, Some("가람"), Some(2), "F", "○○시 사온로 708");
    // 주소 자체가 없는 학생
    add_at(&db, 2026, "주소없는학생", 1, Some("가람"), Some(3), "M", "");

    let t = db.read(|c| by_address(c, &year(2026), today())).unwrap();
    let row = |name: &str| {
        t.rows
            .iter()
            .find(|r| r.name == name)
            .unwrap_or_else(|| panic!("{name} 줄이 없다"))
    };

    assert_eq!(row("기타").total, 1, "사람이 정한 분류다");
    assert_eq!(row("미분류").total, 1, "아직 정하지 못한 상태다");
    assert_eq!(row("주소 없음").total, 1, "주소가 아예 없다");
    assert_eq!(row("기타").bucket, AddressBucket::Category);
    assert_eq!(row("미분류").bucket, AddressBucket::Unclassified);
    assert_eq!(row("주소 없음").bucket, AddressBucket::NoAddress);

    assert_consistent(&db, &year(2026));
}

#[test]
fn 주소_교차표의_행과_열_합계가_전체와_맞는다() {
    let db = db();
    let danji = make_category(&db, "5단지");

    // 1·2·3학년에 흩어 놓는다
    for (i, g) in [1, 1, 2, 3, 3, 3].iter().enumerate() {
        let id = add_at(
            &db,
            2026,
            &format!("단지학생{i}"),
            *g,
            Some("가람"),
            Some(i as i32 + 1),
            "M",
            "○○시 가온로 101",
        );
        set_manual(&db, id, danji);
    }
    add_at(&db, 2026, "미분류학생", 2, Some("나리"), Some(9), "F", "○○시 사온로 707");
    add_at(&db, 2026, "주소없는학생", 4, Some("나리"), Some(9), "F", "");

    let t = db.read(|c| by_address(c, &year(2026), today())).unwrap();
    assert_eq!(t.grades, vec![1, 2, 3, 4]);
    assert_eq!(t.total, 8);

    // 행 합계 = 그 줄의 학년별 합
    for r in &t.rows {
        assert_eq!(
            r.by_grade.iter().sum::<i64>(),
            r.total,
            "{} 줄의 행 합계가 맞지 않는다",
            r.name
        );
    }
    // 열 합계 = 그 학년의 모든 줄 합
    for (i, g) in t.grades.iter().enumerate() {
        let col: i64 = t.rows.iter().map(|r| r.by_grade[i]).sum();
        assert_eq!(col, t.grade_totals[i], "{g}학년 열 합계가 맞지 않는다");
    }
    assert_eq!(t.grade_totals.iter().sum::<i64>(), t.total);

    let danji_row = t.rows.iter().find(|r| r.name == "5단지").unwrap();
    assert_eq!(danji_row.total, 6);
    assert_eq!(danji_row.by_grade, vec![2, 1, 3, 0]);

    assert_consistent(&db, &year(2026));
}

#[test]
fn 학생이_없는_분류도_영으로_보여_준다() {
    let db = db();
    make_category(&db, "빈단지");
    add_at(&db, 2026, "가학생", 1, Some("가람"), Some(1), "M", "");

    let t = db.read(|c| by_address(c, &year(2026), today())).unwrap();
    let empty = t.rows.iter().find(|r| r.name == "빈단지").unwrap();
    assert_eq!(empty.total, 0, "0명도 알려 주는 것이 정보다");
    assert_eq!(empty.by_grade, vec![0]);
}

#[test]
fn 주소_판정_상태를_함께_센다() {
    let db = db();
    let danji = make_category(&db, "5단지");
    let a = add_at(&db, 2026, "직접학생", 1, Some("가람"), Some(1), "M", "○○시 가온로 101");
    set_manual(&db, a, danji);
    add_at(&db, 2026, "미분류학생", 1, Some("가람"), Some(2), "F", "○○시 사온로 707");
    add_at(&db, 2026, "주소없는학생", 1, Some("가람"), Some(3), "M", "");

    let q = db.read(|c| address_quality(c, &year(2026), today())).unwrap();
    assert_eq!(q.total, 3);
    assert_eq!(q.classified, 1);
    assert_eq!(q.unclassified, 1);
    assert_eq!(q.no_address, 1);
    assert_eq!(q.manual, 1, "직접 지정한 학생");
    assert_eq!(
        q.classified + q.unclassified + q.no_address,
        q.total,
        "세 갈래가 겹치지도 빠지지도 않는다"
    );
}

// ---------------------------------------------------------------
// 조건을 걸어도 모든 표가 같은 모집단을 본다
// ---------------------------------------------------------------

#[test]
fn 학년을_고르면_모든_표가_그_학년만_본다() {
    let db = db();
    for g in [1, 2, 3] {
        for i in 1..=5 {
            add(&db, &format!("학생{g}{i}"), g, "가람", i, "M");
        }
    }
    let f = StatFilter {
        grade: Some(2),
        ..year(2026)
    };
    let ck = assert_consistent(&db, &f);
    assert_eq!(ck.total, 5, "2학년만 센다");
    assert_eq!(db.read(|c| grades_present(c, &f, today())).unwrap(), vec![2]);
}

#[test]
fn 주소_분류를_고르면_모든_표가_그_분류만_본다() {
    let db = db();
    let danji = make_category(&db, "5단지");
    for i in 1..=4 {
        let id = add(&db, &format!("단지학생{i}"), 3, "나리", i, "M");
        set_manual(&db, id, danji);
    }
    for i in 5..=9 {
        add(&db, &format!("그냥학생{i}"), 3, "나리", i, "F");
    }

    let f = StatFilter {
        address_category_id: Some(danji),
        ..year(2026)
    };
    let ck = assert_consistent(&db, &f);
    assert_eq!(ck.total, 4);

    // §14 — 주소 분류 + 학년 + 반 까지 좁힐 수 있다
    let narrow = StatFilter {
        grade: Some(3),
        address_category_id: Some(danji),
        ..year(2026)
    };
    let rows = db.read(|c| by_class(c, &narrow, today())).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].class_label, "3-나리");
    assert_eq!(rows[0].counts.total, 4);
}

#[test]
fn 주소_없음만_따로_볼_수_있다() {
    let db = db();
    add_at(&db, 2026, "가학생", 1, Some("가람"), Some(1), "M", "");
    add_at(&db, 2026, "나학생", 1, Some("가람"), Some(2), "F", "");
    add_at(&db, 2026, "다학생", 1, Some("가람"), Some(3), "M", "○○시 가온로 101");

    let f = StatFilter {
        address_none: true,
        ..year(2026)
    };
    assert_eq!(db.read(|c| totals(c, &f, today())).unwrap().total, 2);

    let f = StatFilter {
        address_unclassified: true,
        ..year(2026)
    };
    assert_eq!(
        db.read(|c| totals(c, &f, today())).unwrap().total,
        1,
        "주소는 있는데 분류를 못 정한 학생"
    );
}

#[test]
fn 통계에서_센_숫자가_학생명단_인원과_같다() {
    let db = db();
    let danji = make_category(&db, "5단지");
    for i in 1..=6 {
        let id = add(&db, &format!("단지학생{i}"), 3, "나리", i, "M");
        set_manual(&db, id, danji);
    }
    add_at(&db, 2026, "주소없는학생", 3, Some("나리"), Some(9), "F", "");
    let gone = add(&db, "나간학생", 3, "나리", 10, "M");
    set_manual(&db, gone, danji);
    leave(&db, gone);

    // 통계
    let f = StatFilter {
        grade: Some(3),
        address_category_id: Some(danji),
        ..year(2026)
    };
    let stat = db.read(|c| totals(c, &f, today())).unwrap().total;
    assert_eq!(stat, 6, "전출한 학생은 빠진다");

    // 같은 조건으로 학생명단을 열면 같은 인원이어야 한다
    let listed = db
        .read(|c| {
            let lf = student::ListFilter {
                school_year: 2026,
                grade: Some(3),
                address_category_id: Some(danji),
                ..Default::default()
            };
            Ok(student::list(c, &lf, 500, 0, today())?.total)
        })
        .unwrap();
    assert_eq!(listed, stat, "통계 숫자를 눌러 간 명단이 달라지면 안 된다");

    // 주소 없음도 마찬가지
    let none_stat = db
        .read(|c| {
            totals(
                c,
                &StatFilter {
                    address_none: true,
                    ..year(2026)
                },
                today(),
            )
        })
        .unwrap()
        .total;
    let none_listed = db
        .read(|c| {
            let lf = student::ListFilter {
                school_year: 2026,
                address_none: true,
                ..Default::default()
            };
            Ok(student::list(c, &lf, 500, 0, today())?.total)
        })
        .unwrap();
    assert_eq!(none_stat, 1);
    assert_eq!(none_listed, none_stat);
}

// ---------------------------------------------------------------
// 전입 · 전출이 바로 반영되는가
// ---------------------------------------------------------------

#[test]
fn 전입하면_늘고_전출하면_줄고_돌아오면_다시_는다() {
    let db = db();
    for i in 1..=13 {
        add(&db, &format!("남학생{i:02}"), 3, "나리", i, "M");
    }
    let males = |db: &Db| find(&counts(db, 3), "3-나리").counts.male;
    assert_eq!(males(&db), 13);

    // 전입 +1
    let inp = transfer::TransferInInput {
        student_id: None,
        student: student::StudentInput {
            name: "새학생".into(),
            gender: Some("M".into()),
            birth_raw: Some("170315".into()),
            school_year: 2026,
            grade: 3,
            class_name: Some("나리".into()),
            class_no: Some(25),
            ..Default::default()
        },
        date: "2026-09-10".into(),
    };
    let out = db.write(|c| transfer::transfer_in(c, &inp, today())).unwrap();
    assert_eq!(males(&db), 14, "전입생은 바로 센다");

    // 전출 -1
    leave(&db, out.student_id);
    assert_eq!(males(&db), 13, "전출하면 바로 빠진다");

    // 재전입 +1
    let back = transfer::TransferInInput {
        student_id: Some(out.student_id),
        // 나간 날보다 뒤여야 한다
        date: "2026-09-14".into(),
        ..inp
    };
    db.write(|c| transfer::transfer_in(c, &back, today()))
        .unwrap();
    assert_eq!(males(&db), 14, "돌아오면 다시 센다");

    assert_consistent(&db, &year(2026));
}

// ---------------------------------------------------------------
// 지난 학년도
// ---------------------------------------------------------------

#[test]
fn 지난_학년도는_그때의_학년과_반으로_센다() {
    let db = db();
    // 2025학년도에 2-가람이던 학생
    let id = add_at(
        &db,
        2025,
        "올라간학생",
        2,
        Some("가람"),
        Some(7),
        "M",
        "○○시 가온로 101",
    );
    // 2026학년도에는 3-나리
    db.write(|c| {
        student::ensure_enrollment(c, id, 2026, 3, Some("나리"), Some(12), "MANUAL")?;
        Ok(())
    })
    .unwrap();

    let g2025 = db.read(|c| by_grade(c, &year(2025), today())).unwrap();
    assert_eq!(g2025.len(), 1);
    assert_eq!(g2025[0].grade, 2, "지난 학년도에는 2학년으로 센다");

    let g2026 = db.read(|c| by_grade(c, &year(2026), today())).unwrap();
    assert_eq!(g2026[0].grade, 3, "올해는 3학년으로 센다");

    let c2025 = db.read(|c| by_class(c, &year(2025), today())).unwrap();
    assert_eq!(c2025[0].class_label, "2-가람");
    let c2026 = db.read(|c| by_class(c, &year(2026), today())).unwrap();
    assert_eq!(c2026[0].class_label, "3-나리");
}

#[test]
fn 지난_학년도는_그_해_마지막_상태로_센다() {
    let db = db();
    let stayed = add_at(&db, 2025, "남은학생", 2, Some("가람"), Some(1), "M", "");
    let left = add_at(&db, 2025, "나간학생", 2, Some("가람"), Some(2), "F", "");

    // 2025학년도 안에서 전출했다
    db.write(|c| {
        Ok(c.execute(
            "UPDATE enrollments SET status = 'TRANSFER_OUT', transfer_out_date = '2025-06-01'
              WHERE student_id = ?1 AND school_year = 2025",
            [left],
        )?)
    })
    .unwrap();

    let t = db.read(|c| totals(c, &year(2025), today())).unwrap();
    assert_eq!(
        t.total, 1,
        "학년도 최종 재적 기준 — 그 해 끝에 남아 있던 학생만 센다"
    );
    assert_eq!(stayed > 0, true);
    assert_consistent(&db, &year(2025));
}

#[test]
fn 올해_학적이_없는_학생은_올해_통계에_들지_않는다() {
    let db = db();
    add_at(&db, 2025, "지난해만학생", 6, Some("가람"), Some(1), "M", "");
    add(&db, "올해학생", 1, "가람", 1, "F");

    assert_eq!(db.read(|c| totals(c, &year(2026), today())).unwrap().total, 1);
    assert_eq!(db.read(|c| totals(c, &year(2025), today())).unwrap().total, 1);
}

// ---------------------------------------------------------------
// 여러 상태를 섞어도
// ---------------------------------------------------------------

#[test]
fn 여러_상태가_섞여도_모든_표가_같은_숫자를_말한다() {
    let db = db();
    let danji = make_category(&db, "5단지");

    // 일반 재학생
    for i in 1..=5 {
        let id = add(&db, &format!("일반학생{i}"), 1, "가람", i, "M");
        if i % 2 == 0 {
            set_manual(&db, id, danji);
        }
    }
    // 번호 없음 · 반 없음 · 주소 없음 · 성별 미입력
    add_at(&db, 2026, "번호없음", 2, Some("나리"), None, "F", "○○시 가온로 101");
    add_at(&db, 2026, "반없음", 2, None, Some(3), "", "○○시 가온로 101");
    add_at(&db, 2026, "주소없음", 3, Some("다솜"), Some(1), "M", "");
    add_at(&db, 2026, "미분류", 3, Some("다솜"), Some(2), "F", "○○시 사온로 707");

    // 전입생
    let inp = transfer::TransferInInput {
        student_id: None,
        student: student::StudentInput {
            name: "전입학생".into(),
            gender: Some("M".into()),
            school_year: 2026,
            grade: 4,
            class_name: Some("가람".into()),
            class_no: Some(1),
            ..Default::default()
        },
        date: "2026-09-10".into(),
    };
    db.write(|c| transfer::transfer_in(c, &inp, today())).unwrap();

    // 전출생 (세면 안 된다)
    let gone = add(&db, "전출학생", 5, "가람", 1, "F");
    leave(&db, gone);

    // 전출 후 재전입 (세야 한다)
    let back = add(&db, "돌아온학생", 6, "가람", 1, "M");
    leave(&db, back);
    let again = transfer::TransferInInput {
        student_id: Some(back),
        student: student::StudentInput {
            name: "돌아온학생".into(),
            gender: Some("M".into()),
            school_year: 2026,
            grade: 6,
            class_name: Some("가람".into()),
            class_no: Some(1),
            ..Default::default()
        },
        date: "2026-09-14".into(),
    };
    db.write(|c| transfer::transfer_in(c, &again, today()))
        .unwrap();

    let ck = assert_consistent(&db, &year(2026));
    assert_eq!(ck.total, 5 + 4 + 1 + 1, "전출한 한 명만 빠진다");

    // 명단과도 같아야 한다
    let listed = db
        .read(|c| {
            let lf = student::ListFilter {
                school_year: 2026,
                ..Default::default()
            };
            Ok(student::list(c, &lf, 500, 0, today())?.total)
        })
        .unwrap();
    assert_eq!(listed, ck.total, "학생명단과 통계가 같은 수를 말해야 한다");
}

// ---------------------------------------------------------------
// 빠른가
// ---------------------------------------------------------------

#[test]
fn 이천명도_바로_센다() {
    use std::time::Instant;

    let db = db();
    let danji: Vec<i64> = ["1단지", "2단지", "5단지"]
        .iter()
        .map(|n| make_category(&db, n))
        .collect();

    // 실제 학교보다 큰 2,000명. 주소·성별·반을 골고루 섞는다.
    db.write(|c| {
        for i in 0..2000 {
            let grade = (i % 6) + 1;
            let class = ["가람", "나리", "다솜", "1", "2"][(i % 5) as usize];
            let gender = match i % 7 {
                0 => None,
                n if n % 2 == 0 => Some("M".to_string()),
                _ => Some("F".to_string()),
            };
            let address = match i % 5 {
                0 => None,
                _ => Some(format!("○○시 가온로 {}", 100 + i % 40)),
            };
            let input = student::StudentInput {
                name: format!("학생{i:04}"),
                gender,
                birth_raw: Some("170315".into()),
                address_raw: address,
                school_year: 2026,
                grade,
                class_name: Some(class.to_string()),
                class_no: Some((i / 5) % 40 + 1),
                ..Default::default()
            };
            student::create(c, &input, today())?;
        }
        // 일부에게 주소 분류를 준다
        for i in 0..900i64 {
            addr_repo::set_manual(c, i + 1, Some(danji[(i % 3) as usize]))?;
        }
        Ok(())
    })
    .unwrap();

    let f = year(2026);
    let start = Instant::now();
    let ck = db.read(|c| check_consistency(c, &f, today())).unwrap();
    let elapsed = start.elapsed();

    assert_eq!(ck.total, 2000);
    assert!(ck.ok, "2,000명에서도 모든 표의 합계가 맞는다");
    assert!(
        elapsed.as_millis() < 1000,
        "통계를 한 번 세는 데 {elapsed:?} 걸렸다 — 화면이 바로 열려야 한다"
    );
    println!("2,000명 통계 전체: {elapsed:?}");
}
