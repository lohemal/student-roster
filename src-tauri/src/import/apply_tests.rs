//! 실제로 저장하는 단계 검사.
//!
//! 1,200명 규모와 '같은 파일 두 번' 을 여기서 확인한다.

use super::*;
use crate::db::Db;
use crate::import::mapping::{Field, Mapping};
use crate::import::row;
use crate::repo::{issue, settings, student};

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

fn mapping() -> Mapping {
    let mut m = Mapping::new();
    m.insert(Field::Grade, 0);
    m.insert(Field::ClassName, 1);
    m.insert(Field::ClassNo, 2);
    m.insert(Field::Name, 3);
    m.insert(Field::Gender, 4);
    m.insert(Field::Birth, 5);
    m.insert(Field::MotherPhone, 6);
    m.insert(Field::Address, 7);
    m
}

fn rows(raw: &[Vec<String>]) -> Vec<ParsedRow> {
    raw.iter()
        .enumerate()
        .map(|(i, cells)| row::parse(i + 2, cells, &mapping(), None, today()))
        .collect()
}

fn line(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn meta() -> Meta {
    Meta {
        file_name: "학생명단.xlsx".into(),
        sheet_name: "Sheet1".into(),
        school_year: 2026,
        mapping_json: "{}".into(),
    }
}

fn apply(db: &Db, raw: &[Vec<String>]) -> ApplyResult {
    let rows = rows(raw);
    db.write(|c| run(c, &rows, &meta(), &Choice::default(), today(), |_, _, _, _| {}))
        .unwrap()
}

fn count_students(db: &Db) -> i64 {
    db.read(|c| Ok(c.query_row("SELECT COUNT(*) FROM students", [], |r| r.get(0))?))
        .unwrap()
}

// ---------------------------------------------------------------
// 기본
// ---------------------------------------------------------------

#[test]
fn 새_학생을_저장하고_학적과_사건까지_남긴다() {
    let db = db();
    let out = apply(
        &db,
        &[line(&["3", "가람", "7", "홍길동", "남", "170315", "010-1234-5678", "○○시 ○○로 1"])],
    );

    assert_eq!((out.added, out.updated, out.skipped), (1, 0, 0));
    assert_eq!(count_students(&db), 1);

    let id: i64 = db
        .read(|c| Ok(c.query_row("SELECT id FROM students", [], |r| r.get(0))?))
        .unwrap();
    let d = db.read(|c| student::detail(c, id, 2026)).unwrap();
    assert_eq!(d.name, "홍길동");
    assert_eq!(d.birth_date.as_deref(), Some("2017-03-15"));
    assert_eq!(d.mother_phone.as_deref(), Some("010-1234-5678"), "저장하며 정리된다");
    let e = d.enrollment.expect("학적이 생긴다");
    assert_eq!((e.grade, e.class_name.as_deref(), e.class_no), (3, Some("가람"), Some(7)));
    assert_eq!(d.events.len(), 1, "등록 사건이 남는다");
    assert_eq!(d.events[0].kind, "ENROLL");
}

#[test]
fn 가져오기_기록을_남긴다() {
    let db = db();
    let out = apply(&db, &[line(&["3", "가람", "7", "홍길동", "남", "170315"])]);

    let (file, sheet, year, mode, total, added): (String, String, i32, String, i64, i64) = db
        .read(|c| {
            Ok(c.query_row(
                "SELECT file_name, sheet_name, school_year, mode, total, added
                   FROM imports WHERE id = ?1",
                [out.import_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
            )?)
        })
        .unwrap();

    assert_eq!(file, "학생명단.xlsx");
    assert_eq!(sheet, "Sheet1");
    assert_eq!(year, 2026);
    assert_eq!(mode, "BOTH");
    assert_eq!((total, added), (1, 1));
}

#[test]
fn 확인_필요가_함께_만들어진다() {
    let db = db();
    // 성별·연락처·주소가 없는 줄
    apply(&db, &[line(&["3", "가람", "", "홍길동", "", "20170230"])]);

    let kinds: Vec<String> = db
        .read(|c| {
            let mut st = c.prepare("SELECT kind FROM issues WHERE status = 'OPEN' ORDER BY kind")?;
            let v: Vec<String> = st.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
            Ok(v)
        })
        .unwrap();

    assert!(kinds.contains(&"BIRTH".to_string()), "{kinds:?}");
    assert!(kinds.contains(&"MISSING".to_string()), "{kinds:?}");
    assert!(kinds.contains(&"CLASS_ASSIGN".to_string()), "{kinds:?}");
}

// ---------------------------------------------------------------
// 같은 파일 두 번 — 요구사항의 핵심
// ---------------------------------------------------------------

fn sample_file(n: usize) -> Vec<Vec<String>> {
    (0..n)
        .map(|i| {
            let grade = (i % 6) + 1;
            let no = (i / 6) + 1;
            line(&[
                &grade.to_string(),
                "가람",
                &no.to_string(),
                &format!("학생{i:04}"),
                if i % 2 == 0 { "남" } else { "여" },
                &format!("2017{:02}{:02}", (i % 12) + 1, (i % 28) + 1),
                &format!("010-{:04}-{:04}", i % 10000, (i * 7) % 10000),
                "○○시 ○○로 1",
            ])
        })
        .collect()
}

#[test]
fn 같은_파일을_두_번_넣어도_학생이_늘지_않는다() {
    let db = db();
    let file = sample_file(50);

    let first = apply(&db, &file);
    assert_eq!(first.added, 50);
    assert_eq!(count_students(&db), 50);

    let second = apply(&db, &file);
    assert_eq!(second.added, 0, "같은 학생을 또 만들면 안 된다");
    assert_eq!(second.unchanged, 50);
    assert_eq!(count_students(&db), 50, "학생 수가 그대로여야 한다");
}

#[test]
fn 연락처만_바뀐_파일은_그것만_고친다() {
    let db = db();
    let mut file = sample_file(5);
    apply(&db, &file);

    file[2][6] = "010-9999-8888".to_string();
    let out = apply(&db, &file);

    assert_eq!((out.added, out.updated, out.unchanged), (0, 1, 4));
    assert_eq!(count_students(&db), 5);

    let phone: String = db
        .read(|c| {
            Ok(c.query_row(
                "SELECT mother_phone FROM students WHERE name = '학생0002'",
                [],
                |r| r.get(0),
            )?)
        })
        .unwrap();
    assert_eq!(phone, "010-9999-8888");
}

#[test]
fn 엑셀_빈칸이_기존_연락처를_지우지_않는다() {
    let db = db();
    let mut file = sample_file(3);
    apply(&db, &file);

    for row in file.iter_mut() {
        row[6] = String::new(); // 연락처 열을 통째로 비운다
    }
    let out = apply(&db, &file);

    assert_eq!(out.updated, 0, "빈칸 때문에 고칠 것이 생기면 안 된다");
    let phones: Vec<Option<String>> = db
        .read(|c| {
            let mut st = c.prepare("SELECT mother_phone FROM students ORDER BY name")?;
            let v: Vec<Option<String>> =
                st.query_map([], |r| r.get(0))?.collect::<rusqlite::Result<_>>()?;
            Ok(v)
        })
        .unwrap();
    assert!(phones.iter().all(|p| p.is_some()), "연락처가 살아 있어야 한다: {phones:?}");
}

// ---------------------------------------------------------------
// 사용자가 고른 것만 적용
// ---------------------------------------------------------------

#[test]
fn 신규만_넣고_갱신은_빼는_것이_된다() {
    let db = db();
    let mut file = sample_file(3);
    apply(&db, &file);

    file[0][6] = "010-0000-1111".to_string();
    file.push(line(&["2", "나리", "9", "새학생", "여", "20180101", "010-5-5", "주소"]));

    let rows = rows(&file);
    let choice = Choice {
        add: true,
        update: false,
        skip_rows: Vec::new(),
    };
    let out = db
        .write(|c| run(c, &rows, &meta(), &choice, today(), |_, _, _, _| {}))
        .unwrap();

    assert_eq!(out.added, 1);
    assert_eq!(out.updated, 0);
    assert_eq!(out.skipped, 1, "갱신 대상은 건너뛴 것으로 센다");
}

#[test]
fn 사용자가_뺀_줄은_넣지_않는다() {
    let db = db();
    let file = sample_file(3);
    let rows = rows(&file);
    let skip = rows[1].excel_row;

    let choice = Choice {
        add: true,
        update: true,
        skip_rows: vec![skip],
    };
    let out = db
        .write(|c| run(c, &rows, &meta(), &choice, today(), |_, _, _, _| {}))
        .unwrap();

    assert_eq!(out.added, 2);
    assert_eq!(out.skipped, 1);
    assert_eq!(count_students(&db), 2);
}

#[test]
fn 중복_의심은_저절로_들어가지_않는다() {
    let db = db();
    apply(&db, &[line(&["3", "가람", "1", "김민준", "남", "170315", "010-1-1", "주소"])]);

    // 같은 이름인데 생년월일이 없어 가릴 수 없는 줄
    let out = apply(&db, &[line(&["5", "다솜", "9", "김민준", "남", "", "010-2-2", "주소"])]);

    assert_eq!(out.added, 0);
    assert_eq!(out.updated, 0);
    assert_eq!(out.skipped, 1);
    assert_eq!(count_students(&db), 1, "애매한 것을 마음대로 넣지 않는다");
}

// ---------------------------------------------------------------
// 많은 줄 · 진행 상황 · 트랜잭션
// ---------------------------------------------------------------

#[test]
fn 천이백명을_빠짐없이_넣는다() {
    let db = db();
    let file = sample_file(1200);

    let started = std::time::Instant::now();
    let out = apply(&db, &file);
    let took = started.elapsed();

    assert_eq!(out.total, 1200);
    assert_eq!(out.added, 1200);
    assert_eq!(count_students(&db), 1200);

    let enrolled: i64 = db
        .read(|c| {
            Ok(c.query_row(
                "SELECT COUNT(*) FROM enrollments WHERE school_year = 2026",
                [],
                |r| r.get(0),
            )?)
        })
        .unwrap();
    assert_eq!(enrolled, 1200, "학적도 빠짐없이 생긴다");

    // 시간을 못박지는 않되, 터무니없이 느려지면 알아차리게 해 둔다
    assert!(took.as_secs() < 30, "1,200명에 {took:?} 가 걸렸다");
}

#[test]
fn 천이백명을_다시_넣어도_늘지_않는다() {
    let db = db();
    let file = sample_file(1200);
    apply(&db, &file);

    let again = apply(&db, &file);
    assert_eq!(again.added, 0);
    assert_eq!(again.unchanged, 1200);
    assert_eq!(count_students(&db), 1200);
}

#[test]
fn 진행_상황이_단계별로_올라간다() {
    let db = db();
    let rows = rows(&sample_file(30));

    let mut seen: Vec<(String, usize, usize)> = Vec::new();
    db.write(|c| {
        run(c, &rows, &meta(), &Choice::default(), today(), |stage, _, done, total| {
            seen.push((stage.to_string(), done, total))
        })
    })
    .unwrap();

    let stages: Vec<&str> = seen.iter().map(|(s, _, _)| s.as_str()).collect();
    assert!(stages.contains(&"ANALYZE"), "{stages:?}");
    assert!(stages.contains(&"SAVE"), "{stages:?}");
    assert!(stages.contains(&"FINISH"), "{stages:?}");
    assert!(seen.iter().all(|(_, d, t)| d <= t), "진행이 전체를 넘지 않는다");

    // 단계 순서가 뒤집히지 않는다
    let first_save = stages.iter().position(|s| *s == "SAVE").unwrap();
    let last_analyze = stages.iter().rposition(|s| *s == "ANALYZE").unwrap();
    assert!(last_analyze < first_save);
}

#[test]
fn 중간에_실패하면_하나도_들어가지_않는다() {
    let db = db();
    let file = sample_file(10);
    let rows = rows(&file);

    // 저장 도중에 일부러 실패시킨다
    let outcome: AppResult<()> = db.write(|c| {
        run(c, &rows, &meta(), &Choice::default(), today(), |_, _, _, _| {})?;
        Err(crate::error::AppError::internal("일부러 낸 오류"))
    });
    assert!(outcome.is_err());

    assert_eq!(count_students(&db), 0, "전부 아니면 전무여야 한다");
    let imports: i64 = db
        .read(|c| Ok(c.query_row("SELECT COUNT(*) FROM imports", [], |r| r.get(0))?))
        .unwrap();
    assert_eq!(imports, 0, "기록도 남지 않는다");
}

// ---------------------------------------------------------------
// 지난 학년도 학생
// ---------------------------------------------------------------

#[test]
fn 지난_학년도_학생에게_올해_학적을_만들어_준다() {
    let db = db();
    db.write(|c| settings::create_year(c, 2027)).unwrap();
    apply(&db, &[line(&["3", "가람", "1", "홍길동", "남", "170315", "010-1-1", "주소"])]);

    let id: i64 = db
        .read(|c| Ok(c.query_row("SELECT id FROM students", [], |r| r.get(0))?))
        .unwrap();

    // 2027 학년도 명단으로 다시 가져온다
    let rows = rows(&[line(&["4", "나리", "5", "홍길동", "남", "170315", "010-1-1", "주소"])]);
    let meta2027 = Meta {
        school_year: 2027,
        ..meta()
    };
    let out = db
        .write(|c| run(c, &rows, &meta2027, &Choice::default(), today(), |_, _, _, _| {}))
        .unwrap();

    assert_eq!(out.added, 0, "같은 사람이니 새로 만들지 않는다");
    assert_eq!(out.updated, 1);
    assert_eq!(count_students(&db), 1);

    let d = db.read(|c| student::detail(c, id, 2027)).unwrap();
    assert_eq!(d.enrollments.len(), 2, "학년도별로 학적이 하나씩");
    let e = d.enrollment.expect("2027 학적");
    assert_eq!((e.grade, e.class_name.as_deref(), e.class_no), (4, Some("나리"), Some(5)));

    // 지난 학년도 기록은 그대로다
    let old = d.enrollments.iter().find(|e| e.school_year == 2026).unwrap();
    assert_eq!(old.class_label, "3-가람");
    assert_eq!(old.class_no, Some(1));
}

#[test]
fn 확인_필요_건수를_결과에_담는다() {
    let db = db();
    let out = apply(
        &db,
        &[
            line(&["3", "가람", "1", "홍길동", "남", "170315", "010-1-1", "주소"]),
            line(&["3", "가람", "", "김영희", "", "", "", ""]),
        ],
    );
    assert_eq!(out.added, 2);
    assert!(out.issue_count > 0, "확인 필요가 실제로 세어져야 한다");

    let real = db.read(|c| issue::summary(c, 2026)).unwrap();
    let total: i64 = real.iter().map(|k| k.count).sum();
    assert_eq!(out.issue_count, total, "결과와 실제 표시가 같아야 한다");
}
