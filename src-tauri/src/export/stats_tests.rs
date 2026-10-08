//! 통계 Excel 검사.
//!
//! 계획만 보지 않는다. **실제로 `.xlsx` 를 만들어 calamine 으로 다시 열고**, 거기
//! 적힌 숫자가 `repo::stats` 가 센 숫자와 같은지 본다. 화면과 파일이 다른 숫자를
//! 말하기 시작하면 둘 다 못 쓰기 때문이다. 이름은 모두 가상이다.

use std::path::PathBuf;

use calamine::{Data, Reader, Xlsx};
use chrono::NaiveDate;

use super::*;
use crate::export::write_plan;
use crate::db::Db;
use crate::repo::stats::{self, StatFilter};
use crate::repo::{settings, student};

fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2027, 4, 1).unwrap()
}

fn tmp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "roster-stats-{tag}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 1학년 남 10 · 여 11, 2학년 남 12 · 여 13. 합계 46명.
///
/// 숫자를 일부러 서로 다르게 둔다 — 열이 밀리면 바로 드러난다.
fn db() -> Db {
    let db = Db::memory();
    db.write(|c| {
        settings::create_year(c, 2027)?;
        settings::set_current_year(c, 2027)
    })
    .unwrap();

    let mut n = 0;
    let mut add = |grade: i32, class_name: &str, gender: &str, count: i32| {
        for i in 1..=count {
            n += 1;
            let input = student::StudentInput {
                name: format!("가온{n:03}"),
                gender: Some(gender.into()),
                birth_raw: Some("170315".into()),
                school_year: 2027,
                grade,
                class_name: Some(class_name.into()),
                class_no: Some(i),
                ..Default::default()
            };
            db.write(|c| student::create(c, &input, today())).unwrap();
        }
    };
    add(1, "가람", "M", 10);
    add(1, "가람", "F", 11);
    add(2, "나리", "M", 12);
    add(2, "나리", "F", 13);
    db
}

/// 화면이 쓰는 것과 **같은** 집계 함수로 값을 모은다.
struct Gathered {
    totals: stats::Counts,
    by_grade: Vec<stats::GradeRow>,
    by_class: Vec<stats::ClassCount>,
    address: stats::AddressTable,
}

fn gather(db: &Db, f: &StatFilter) -> Gathered {
    db.read(|c| {
        Ok(Gathered {
            totals: stats::totals(c, f, today())?,
            by_grade: stats::by_grade(c, f, today())?,
            by_class: stats::by_class(c, f, today())?,
            address: stats::by_address(c, f, today())?,
        })
    })
    .unwrap()
}

impl Gathered {
    fn source(&self, school_year: i32) -> Source<'_> {
        Source {
            school_year,
            totals: &self.totals,
            by_grade: &self.by_grade,
            by_class: &self.by_class,
            address: &self.address,
        }
    }
}

/// 한 칸의 값과 **종류**. 숫자가 글자로 들어갔는지 보려면 종류를 봐야 한다.
#[derive(Debug, Clone, PartialEq)]
enum Cell {
    Text(String),
    Num(f64),
    Empty,
}

impl Cell {
    fn text(&self) -> String {
        match self {
            Cell::Text(s) => s.clone(),
            Cell::Num(n) => n.to_string(),
            Cell::Empty => String::new(),
        }
    }
}

fn write_and_read(plan: &ExportPlan, tag: &str) -> Vec<(String, Vec<Vec<Cell>>)> {
    let dir = tmp_dir(tag);
    let path = dir.join(format!("{}.xlsx", plan.files[0].file_name));
    write_plan(plan, &path).expect("파일을 만들지 못했다");
    assert!(path.exists(), "파일이 생겨야 한다");

    let mut book: Xlsx<_> = calamine::open_workbook(&path).expect("만든 파일을 다시 열지 못했다");
    let names = book.sheet_names().to_vec();
    let out = names
        .into_iter()
        .map(|n| {
            let range = book.worksheet_range(&n).expect("시트를 읽지 못했다");
            let rows: Vec<Vec<Cell>> = range
                .rows()
                .map(|r| {
                    r.iter()
                        .map(|c| match c {
                            Data::Empty => Cell::Empty,
                            Data::String(s) => Cell::Text(s.clone()),
                            Data::Float(f) => Cell::Num(*f),
                            Data::Int(i) => Cell::Num(*i as f64),
                            other => Cell::Text(other.to_string()),
                        })
                        .collect()
                })
                .collect();
            (n, rows)
        })
        .collect();
    let _ = std::fs::remove_dir_all(&dir);
    out
}

fn texts(rows: &[Vec<Cell>]) -> Vec<Vec<String>> {
    rows.iter().map(|r| r.iter().map(Cell::text).collect()).collect()
}

// ---------------------------------------------------------------
// 고른 통계만 시트가 된다
// ---------------------------------------------------------------

#[test]
fn 하나만_고르면_시트도_하나다() {
    let db = db();
    let f = StatFilter::year(2027);
    let g = gather(&db, &f);
    let book = write_and_read(&plan(&g.source(2027), &[Kind::Grade]), "one");

    assert_eq!(book.len(), 1);
    assert_eq!(book[0].0, "학년별");
}

#[test]
fn 두_개를_고르면_둘만_생긴다() {
    let db = db();
    let f = StatFilter::year(2027);
    let g = gather(&db, &f);
    let book = write_and_read(&plan(&g.source(2027), &[Kind::Grade, Kind::Address]), "two");

    let names: Vec<&str> = book.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, vec!["학년별", "주소분류"], "학년반별은 없어야 한다");
}

#[test]
fn 전체를_고르면_셋_다_생기고_차례는_화면과_같다() {
    let db = db();
    let f = StatFilter::year(2027);
    let g = gather(&db, &f);
    // 고른 차례를 뒤집어도 시트 차례는 언제나 같다
    let book = write_and_read(
        &plan(&g.source(2027), &[Kind::Address, Kind::Class, Kind::Grade]),
        "all",
    );

    let names: Vec<&str> = book.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, vec!["학년별", "학년반별", "주소분류"]);
}

// ---------------------------------------------------------------
// 숫자가 화면과 같은가
// ---------------------------------------------------------------

#[test]
fn 학년별_시트의_숫자가_집계와_같다() {
    let db = db();
    let f = StatFilter::year(2027);
    let g = gather(&db, &f);
    let book = write_and_read(&plan(&g.source(2027), &[Kind::Grade]), "grade");
    let rows = texts(&book[0].1);

    assert_eq!(rows[0], vec!["학년", "남", "여", "미입력", "합계"]);
    assert_eq!(rows[1], vec!["1학년", "10", "11", "0", "21"]);
    assert_eq!(rows[2], vec!["2학년", "12", "13", "0", "25"]);
    assert_eq!(rows[3], vec!["합계", "22", "24", "0", "46"], "화면 맨 아랫줄");
    assert_eq!(rows.len(), 4, "머리글 + 학년 둘 + 합계");

    // 집계 결과와 한 번 더 맞춰 본다 — 사람이 적은 숫자가 아니라 센 숫자다
    assert_eq!(g.totals.total, 46);
    for (i, r) in g.by_grade.iter().enumerate() {
        assert_eq!(rows[i + 1][4], r.counts.total.to_string());
    }
}

#[test]
fn 학년반별_시트는_학년과_반을_나눠_적는다() {
    let db = db();
    let f = StatFilter::year(2027);
    let g = gather(&db, &f);
    let book = write_and_read(&plan(&g.source(2027), &[Kind::Class]), "class");
    let rows = texts(&book[0].1);

    assert_eq!(rows[0], vec!["학년", "반", "남", "여", "미입력", "합계"]);
    assert_eq!(rows[1], vec!["1학년", "가람", "10", "11", "0", "21"]);
    assert_eq!(rows[2], vec!["2학년", "나리", "12", "13", "0", "25"]);
    assert_eq!(rows[3][0], "합계");
    assert_eq!(rows[3][5], "46");
}

#[test]
fn 반이_없는_학생은_미정으로_적는다() {
    let db = db();
    db.write(|c| {
        student::create(
            c,
            &student::StudentInput {
                name: "나린별".into(),
                gender: Some("F".into()),
                school_year: 2027,
                grade: 3,
                class_name: None,
                class_no: None,
                ..Default::default()
            },
            today(),
        )
        .map(|_| ())
    })
    .unwrap();

    let f = StatFilter::year(2027);
    let g = gather(&db, &f);
    let book = write_and_read(&plan(&g.source(2027), &[Kind::Class]), "undecided");
    let rows = texts(&book[0].1);

    assert!(
        rows.iter().any(|r| r[0] == "3학년" && r[1] == "미정"),
        "빠뜨리면 반별 합계가 전체와 달라진다: {rows:?}"
    );
    assert_eq!(rows.last().unwrap()[5], "47");
}

#[test]
fn 주소분류_시트는_학년_열과_합계를_그대로_옮긴다() {
    let db = db();
    let f = StatFilter::year(2027);
    let g = gather(&db, &f);
    let book = write_and_read(&plan(&g.source(2027), &[Kind::Address]), "address");
    let rows = texts(&book[0].1);

    let mut want = vec!["주소 분류".to_string()];
    want.extend(g.address.grades.iter().map(|x| format!("{x}학년")));
    want.push("합계".into());
    assert_eq!(rows[0], want);

    // 줄 수 = 분류 수 + 머리글 + 합계
    assert_eq!(rows.len(), g.address.rows.len() + 2);
    assert_eq!(rows.last().unwrap()[0], "합계");
    assert_eq!(
        rows.last().unwrap().last().unwrap(),
        &g.address.total.to_string()
    );
    // 주소를 적지 않았으므로 모두 '주소 없음' 이다
    let no_addr = g
        .address
        .rows
        .iter()
        .find(|r| r.bucket == stats::AddressBucket::NoAddress)
        .unwrap();
    assert_eq!(no_addr.total, 46);
}

// ---------------------------------------------------------------
// 행정자료로 쓸 수 있는 모양인가
// ---------------------------------------------------------------

#[test]
fn 인원은_숫자_칸으로_들어간다() {
    let db = db();
    let f = StatFilter::year(2027);
    let g = gather(&db, &f);
    let book = write_and_read(&plan(&g.source(2027), &[Kind::Grade]), "numeric");
    let rows = &book[0].1;

    assert_eq!(rows[1][0], Cell::Text("1학년".into()), "이름 칸은 글자");
    assert_eq!(rows[1][1], Cell::Num(10.0), "인원은 숫자라야 Excel 에서 더한다");
    assert_eq!(rows[1][4], Cell::Num(21.0));
    assert_eq!(rows[3][4], Cell::Num(46.0), "합계도 숫자");
}

#[test]
fn 파일_이름은_학년도를_앞에_둔다() {
    assert_eq!(file_name(2027), "2027학년도_학생현황통계");
}

// ---------------------------------------------------------------
// 화면 조건이 그대로 간다
// ---------------------------------------------------------------

#[test]
fn 학년_조건을_걸면_그_학년만_들어간다() {
    let db = db();
    let f = StatFilter {
        grade: Some(1),
        ..StatFilter::year(2027)
    };
    let g = gather(&db, &f);
    let book = write_and_read(&plan(&g.source(2027), &[Kind::Grade, Kind::Class]), "filter");

    let grade = texts(&book[0].1);
    assert_eq!(grade.len(), 3, "머리글 + 1학년 + 합계");
    assert_eq!(grade[1][0], "1학년");
    assert_eq!(grade[2], vec!["합계", "10", "11", "0", "21"]);

    let class = texts(&book[1].1);
    assert!(
        class.iter().skip(1).all(|r| r[0] == "1학년" || r[0] == "합계"),
        "2학년이 끼면 안 된다: {class:?}"
    );
}

#[test]
fn 셀_학생이_없어도_파일은_만들어진다() {
    // 학생이 하나도 없는 학년도 — 머리글과 0 짜리 합계만 남는다
    let db = Db::memory();
    db.write(|c| {
        settings::create_year(c, 2027)?;
        settings::set_current_year(c, 2027)
    })
    .unwrap();

    let f = StatFilter::year(2027);
    let g = gather(&db, &f);
    let book = write_and_read(&plan(&g.source(2027), &[Kind::Grade]), "empty");
    let rows = texts(&book[0].1);

    assert_eq!(rows[0][0], "학년");
    assert_eq!(rows[1], vec!["합계", "0", "0", "0", "0"]);
}
