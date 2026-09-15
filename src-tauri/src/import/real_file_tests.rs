//! 진짜 엑셀 파일로 처음부터 끝까지 해 본다.
//!
//! 앞의 검사들은 이미 글자로 바뀐 줄을 다뤘다. 여기서는 `.xlsx` 를 실제로 만들어
//! **읽기 → 헤더 찾기 → 짝짓기 → 분석 → 저장**까지 한 번에 확인한다.
//! 숫자 칸·날짜 칸·앞자리 0이 빠진 연락처처럼 진짜 파일에서만 생기는 일도 함께 본다.

use std::path::PathBuf;

use chrono::NaiveDate;
use rust_xlsxwriter::{Workbook, Worksheet};

use super::{analyze, apply, excel, mapping, row};
use crate::db::Db;
use crate::repo::settings;

fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 9, 15).unwrap()
}

fn tmp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "roster-import-{tag}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
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

/// 학교에서 흔히 쓰는 모양의 명단을 만든다.
///
/// * 맨 윗줄은 제목 — 헤더가 둘째 줄에 있다
/// * 열 이름이 프로그램 항목과 똑같지 않다 (학급·성명·출석번호·보호자 연락처)
/// * 번호와 생년월일은 숫자 칸, 연락처 하나는 앞자리 0이 빠진 숫자 칸
fn write_roster(path: &PathBuf, count: usize, phone_suffix: &str) {
    let mut wb = Workbook::new();
    let sheet: &mut Worksheet = wb.add_worksheet();
    sheet.set_name("학생명단").unwrap();

    sheet.write_string(0, 0, "2026학년도 학생명단").unwrap();

    let headers = [
        "학년", "학급", "출석번호", "성명", "성별", "생년월일",
        "주소", "아버지", "어머니", "부 연락처", "모 연락처", "보호자 연락처", "비고",
    ];
    for (i, h) in headers.iter().enumerate() {
        sheet.write_string(1, i as u16, *h).unwrap();
    }

    let classes = ["가람", "나리", "다솜"];
    for i in 0..count {
        let r = (i + 2) as u32;
        let grade = (i % 6) + 1;
        let class = classes[i % classes.len()];
        let no = (i / 18) + 1;

        sheet.write_number(r, 0, grade as f64).unwrap();
        sheet.write_string(r, 1, class).unwrap();
        sheet.write_number(r, 2, no as f64).unwrap(); // 번호는 숫자 칸
        sheet.write_string(r, 3, &format!("학생{i:04}")).unwrap();
        sheet.write_string(r, 4, if i % 2 == 0 { "남" } else { "여" }).unwrap();
        // 생년월일도 숫자 칸 — 170315 가 170315.0 이 되면 안 된다
        let yy = 17 + (i % 6);
        let mm = (i % 12) + 1;
        let dd = (i % 28) + 1;
        sheet
            .write_number(r, 5, format!("{yy:02}{mm:02}{dd:02}").parse::<f64>().unwrap())
            .unwrap();
        sheet.write_string(r, 6, "○○시 ○○로 1").unwrap();
        sheet.write_string(r, 7, "아버지이름").unwrap();
        sheet.write_string(r, 8, "어머니이름").unwrap();
        // 부 연락처는 앞자리 0이 빠진 숫자 칸
        sheet
            .write_number(r, 9, format!("10{:08}", i % 100000000).parse::<f64>().unwrap())
            .unwrap();
        sheet
            .write_string(r, 10, &format!("010-{:04}-{}", i % 10000, phone_suffix))
            .unwrap();
        sheet.write_string(r, 11, "010-0000-0000").unwrap();
        if i % 20 == 0 {
            sheet.write_string(r, 12, "특이사항").unwrap();
        }
    }
    wb.save(path).unwrap();
}

/// 파일 하나를 분석해서 저장까지 한다.
fn import_file(db: &Db, path: &PathBuf, year: i32) -> apply::ApplyResult {
    let all = excel::read_sheet(path.to_str().unwrap(), "학생명단").unwrap();
    let header_row = mapping::guess_header_row(&all, 10);
    let headers = &all[header_row].1;
    let map = mapping::guess(headers);

    let rows: Vec<_> = all[header_row + 1..]
        .iter()
        .map(|(n, cells)| row::parse(*n, cells, &map, None, today()))
        .collect();

    let meta = apply::Meta {
        file_name: "학생명단.xlsx".into(),
        sheet_name: "학생명단".into(),
        school_year: year,
        mapping_json: serde_json::to_string(&map).unwrap(),
    };
    db.write(|c| {
        apply::run(
            c,
            &rows,
            &meta,
            &apply::Choice::default(),
            today(),
            |_, _, _, _| {},
        )
    })
    .unwrap()
}

// ---------------------------------------------------------------

#[test]
fn 진짜_엑셀_파일에서_제목_줄을_건너뛰고_헤더를_찾는다() {
    let dir = tmp_dir("header");
    let path = dir.join("명단.xlsx");
    write_roster(&path, 5, "1111");

    let all = excel::read_sheet(path.to_str().unwrap(), "학생명단").unwrap();
    let header_row = mapping::guess_header_row(&all, 10);
    assert_eq!(header_row, 1, "첫 줄은 제목이므로 둘째 줄이 헤더다");

    let map = mapping::guess(&all[header_row].1);
    use mapping::Field;
    assert_eq!(map.get(&Field::Name), Some(&3), "'성명' 을 이름으로 알아본다");
    assert_eq!(map.get(&Field::ClassName), Some(&1), "'학급' 을 반으로");
    assert_eq!(map.get(&Field::ClassNo), Some(&2), "'출석번호' 를 번호로");
    assert_eq!(map.get(&Field::PrimaryPhone), Some(&11), "'보호자 연락처' 를 주보호자로");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn 시트_목록을_읽는다() {
    let dir = tmp_dir("sheets");
    let path = dir.join("명단.xlsx");
    write_roster(&path, 3, "1111");

    let info = excel::inspect(path.to_str().unwrap()).unwrap();
    assert_eq!(info.file_name, "명단.xlsx");
    assert_eq!(info.sheets.len(), 1);
    assert_eq!(info.sheets[0].name, "학생명단");
    assert!(info.sheets[0].rows >= 4, "제목+헤더+3줄");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn 숫자_칸으로_저장된_값을_제대로_읽는다() {
    let db = db();
    let dir = tmp_dir("numeric");
    let path = dir.join("명단.xlsx");
    write_roster(&path, 1, "1111");

    import_file(&db, &path, 2026);

    let (birth, no, father): (Option<String>, Option<i32>, Option<String>) = db
        .read(|c| {
            Ok(c.query_row(
                "SELECT s.birth_date, e.class_no, s.father_phone
                   FROM students s JOIN enrollments e ON e.student_id = s.id",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?)
        })
        .unwrap();

    assert_eq!(birth.as_deref(), Some("2017-01-01"), "170101 이 소수로 깨지지 않는다");
    assert_eq!(no, Some(1), "번호가 숫자 칸이어도 읽힌다");
    assert_eq!(
        father.as_deref(),
        Some("010-0000-0000"),
        "앞자리 0이 빠진 1000000000 을 되살려 읽는다"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn 천이백명짜리_파일을_빠짐없이_가져온다() {
    let db = db();
    let dir = tmp_dir("1200");
    let path = dir.join("명단.xlsx");
    write_roster(&path, 1200, "1111");

    let started = std::time::Instant::now();
    let out = import_file(&db, &path, 2026);
    let took = started.elapsed();

    assert_eq!(out.total, 1200);
    assert_eq!(out.added, 1200);
    assert_eq!(out.skipped, 0);

    let (students, enrollments): (i64, i64) = db
        .read(|c| {
            Ok((
                c.query_row("SELECT COUNT(*) FROM students", [], |r| r.get(0))?,
                c.query_row(
                    "SELECT COUNT(*) FROM enrollments WHERE school_year = 2026",
                    [],
                    |r| r.get(0),
                )?,
            ))
        })
        .unwrap();
    assert_eq!(students, 1200);
    assert_eq!(enrollments, 1200);

    assert!(took.as_secs() < 60, "1,200명에 {took:?} 가 걸렸다");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn 같은_파일을_다시_가져와도_학생이_늘지_않는다() {
    let db = db();
    let dir = tmp_dir("again");
    let path = dir.join("명단.xlsx");
    write_roster(&path, 300, "1111");

    let first = import_file(&db, &path, 2026);
    assert_eq!(first.added, 300);

    let second = import_file(&db, &path, 2026);
    assert_eq!(second.added, 0, "두 번째에 300명이 또 생기면 안 된다");
    assert_eq!(second.unchanged, 300);

    let n: i64 = db
        .read(|c| Ok(c.query_row("SELECT COUNT(*) FROM students", [], |r| r.get(0))?))
        .unwrap();
    assert_eq!(n, 300);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn 연락처만_고친_파일은_그_학생만_갱신한다() {
    let db = db();
    let dir = tmp_dir("changed");
    let first_path = dir.join("처음.xlsx");
    let second_path = dir.join("고친것.xlsx");
    write_roster(&first_path, 100, "1111");
    write_roster(&second_path, 100, "9999"); // 모 연락처 끝자리만 다르다

    import_file(&db, &first_path, 2026);
    let out = import_file(&db, &second_path, 2026);

    assert_eq!(out.added, 0);
    assert_eq!(out.updated, 100, "모두 연락처가 바뀌었다");
    assert_eq!(out.unchanged, 0);

    let phone: String = db
        .read(|c| {
            Ok(c.query_row(
                "SELECT mother_phone FROM students WHERE name = '학생0000'",
                [],
                |r| r.get(0),
            )?)
        })
        .unwrap();
    assert_eq!(phone, "010-0000-9999");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn 분석_단계에서는_아무것도_저장되지_않는다() {
    let db = db();
    let dir = tmp_dir("dry");
    let path = dir.join("명단.xlsx");
    write_roster(&path, 50, "1111");

    let all = excel::read_sheet(path.to_str().unwrap(), "학생명단").unwrap();
    let header_row = mapping::guess_header_row(&all, 10);
    let map = mapping::guess(&all[header_row].1);
    let rows: Vec<_> = all[header_row + 1..]
        .iter()
        .map(|(n, cells)| row::parse(*n, cells, &map, None, today()))
        .collect();

    let result = db
        .read(|c| analyze::run(c, &rows, 2026, |_, _| {}))
        .unwrap();
    assert_eq!(result.summary.add, 50);

    let n: i64 = db
        .read(|c| Ok(c.query_row("SELECT COUNT(*) FROM students", [], |r| r.get(0))?))
        .unwrap();
    assert_eq!(n, 0, "분석만 했는데 자료가 들어가면 안 된다");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn 없는_시트를_고르면_알아들을_수_있게_알린다() {
    let dir = tmp_dir("nosheet");
    let path = dir.join("명단.xlsx");
    write_roster(&path, 3, "1111");

    let err = excel::read_sheet(path.to_str().unwrap(), "없는시트").unwrap_err();
    assert_eq!(err.code, "INVALID_INPUT");
    assert!(err.user_message.contains("시트"), "{}", err.user_message);

    let _ = std::fs::remove_dir_all(&dir);
}

/// 사람이 만든 시험용 파일을 읽어 본다.
///
/// `npm run sample` 로 만든 파일이 있을 때만 돌린다.
///   cargo test --lib sample_file -- --ignored --nocapture
#[test]
#[ignore]
fn 시험용_파일을_읽어_본다() {
    let path = std::path::Path::new("../scratch/시험-학생명단.xlsx");
    assert!(path.exists(), "먼저 `npm run sample` 로 파일을 만들어 주세요");
    let p = path.to_str().unwrap();

    let info = excel::inspect(p).unwrap();
    println!("파일: {} / 시트: {:?}", info.file_name, info.sheets);

    let all = excel::read_sheet(p, &info.sheets[0].name).unwrap();
    let header_row = mapping::guess_header_row(&all, 10);
    println!("헤더 줄: {} -> {:?}", header_row + 1, all[header_row].1);

    let map = mapping::guess(&all[header_row].1);
    let mut pairs: Vec<String> = map
        .iter()
        .map(|(f, c)| format!("{} = {}", f.label(), all[header_row].1[*c]))
        .collect();
    pairs.sort();
    println!("짝짓기:\n  {}", pairs.join("\n  "));

    let rows: Vec<_> = all[header_row + 1..]
        .iter()
        .map(|(n, cells)| row::parse(*n, cells, &map, None, today()))
        .collect();
    println!("자료 {}줄", rows.len());
    println!("첫 줄: {:?}", rows[0]);

    let db = db();
    let out = import_file(&db, &path.to_path_buf(), 2026);
    println!("가져오기 결과: {out:?}");
    assert_eq!(out.added + out.skipped, rows.len());
}
