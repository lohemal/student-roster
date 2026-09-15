//! 기존 학생과 견주는 판정 검사.
//!
//! 여기가 가장 위험한 곳이다 — 잘못 판정하면 남의 자료를 덮어쓴다.
//! 그래서 '확실할 때만 갱신, 애매하면 중복 의심' 이 지켜지는지 꼼꼼히 본다.

use super::*;
use crate::db::Db;
use crate::import::mapping::{Field, Mapping};
use crate::import::row;
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

/// 학년·반·번호·이름·성별·생년월일·부연락처·모연락처·주소
fn mapping() -> Mapping {
    let mut m = Mapping::new();
    m.insert(Field::Grade, 0);
    m.insert(Field::ClassName, 1);
    m.insert(Field::ClassNo, 2);
    m.insert(Field::Name, 3);
    m.insert(Field::Gender, 4);
    m.insert(Field::Birth, 5);
    m.insert(Field::FatherPhone, 6);
    m.insert(Field::MotherPhone, 7);
    m.insert(Field::Address, 8);
    m
}

fn rows(raw: &[&[&str]]) -> Vec<ParsedRow> {
    raw.iter()
        .enumerate()
        .map(|(i, cells)| {
            let cells: Vec<String> = cells.iter().map(|s| s.to_string()).collect();
            row::parse(i + 2, &cells, &mapping(), None, today())
        })
        .collect()
}

fn analyze(db: &Db, raw: &[&[&str]]) -> AnalyzeResult {
    let rows = rows(raw);
    db.read(|c| run(c, &rows, 2026, |_, _| {})).unwrap()
}

/// 이미 등록된 학생 하나를 만든다.
fn seed(db: &Db, name: &str, birth: &str, grade: i32, class: &str, no: i32) -> i64 {
    let input = student::StudentInput {
        name: name.into(),
        gender: Some("M".into()),
        birth_raw: if birth.is_empty() { None } else { Some(birth.into()) },
        mother_phone: Some("010-1234-5678".into()),
        address_raw: Some("○○시 ○○로 1".into()),
        school_year: 2026,
        grade,
        class_name: Some(class.into()),
        class_no: Some(no),
        ..Default::default()
    };
    db.write(|c| student::create(c, &input, today())).unwrap()
}

// ---------------------------------------------------------------
// 새 학생
// ---------------------------------------------------------------

#[test]
fn 빈_자료에_넣으면_모두_신규다() {
    let db = db();
    let r = analyze(
        &db,
        &[
            &["3", "가람", "1", "홍길동", "남", "170315"],
            &["3", "가람", "2", "김영희", "여", "170820"],
        ],
    );
    assert_eq!(r.summary.add, 2);
    assert_eq!(r.summary.update, 0);
    assert_eq!(r.summary.ambiguous, 0);
    assert!(r.rows.iter().all(|p| p.action == Action::Add));
}

#[test]
fn 이름이_없으면_가져올_수_없음으로_센다() {
    let db = db();
    let r = analyze(&db, &[&["3", "가람", "1", ""], &["3", "가람", "2", "김영희"]]);
    assert_eq!(r.summary.blocked, 1);
    assert_eq!(r.summary.add, 1);
    assert!(r.rows[0].reason.as_deref().unwrap().contains("이름"));
}

// ---------------------------------------------------------------
// 같은 파일을 다시 가져오기 — 요구사항의 핵심
// ---------------------------------------------------------------

#[test]
fn 같은_파일을_다시_분석하면_신규가_되지_않는다() {
    let db = db();
    let file: &[&[&str]] = &[
        &["3", "가람", "1", "홍길동", "남", "170315", "010-1111-2222"],
        &["3", "가람", "2", "김영희", "여", "170820", "010-3333-4444"],
    ];

    // 처음 — 둘 다 신규
    assert_eq!(analyze(&db, file).summary.add, 2);

    // 실제로 넣는다
    for r in rows(file) {
        db.write(|c| student::create(c, &row::to_input(&r, 2026), today()))
            .unwrap();
    }

    // 다시 — 하나도 신규가 아니고, 바뀐 것도 없다
    let again = analyze(&db, file);
    assert_eq!(again.summary.add, 0, "같은 학생을 또 만들면 안 된다");
    assert_eq!(again.summary.unchanged, 2);
    assert_eq!(again.summary.update, 0);
}

#[test]
fn 값이_바뀌었으면_갱신으로_보고_무엇이_바뀌는지_알려_준다() {
    let db = db();
    seed(&db, "홍길동", "170315", 3, "가람", 1);

    let r = analyze(
        &db,
        &[&["3", "가람", "1", "홍길동", "남", "170315", "", "010-9876-5432"]],
    );
    assert_eq!(r.summary.update, 1);

    let plan = &r.rows[0];
    let change = plan
        .changes
        .iter()
        .find(|c| c.field == "모 연락처")
        .expect("모 연락처가 바뀌어야 한다");
    assert_eq!(change.before, "010-1234-5678");
    assert_eq!(change.after, "010-9876-5432");
}

#[test]
fn 겉모양만_다른_연락처는_바뀐_것으로_보지_않는다() {
    let db = db();
    seed(&db, "홍길동", "170315", 3, "가람", 1);

    // 저장된 값은 010-1234-5678. 엑셀에는 하이픈 없이 들어 있다
    let r = analyze(
        &db,
        &[&["3", "가람", "1", "홍길동", "남", "170315", "", "01012345678"]],
    );
    assert_eq!(r.summary.unchanged, 1, "같은 번호인데 갱신으로 보면 안 된다");
}

// ---------------------------------------------------------------
// 엑셀 빈칸이 기존 값을 지우지 않는다
// ---------------------------------------------------------------

#[test]
fn 엑셀_빈칸은_기존_연락처를_지우지_않는다() {
    let db = db();
    seed(&db, "홍길동", "170315", 3, "가람", 1);

    let r = analyze(&db, &[&["3", "가람", "1", "홍길동", "남", "170315", "", ""]]);
    assert_eq!(r.summary.unchanged, 1, "빈칸 때문에 갱신이 생기면 안 된다");
    assert!(r.rows[0].changes.is_empty());
}

#[test]
fn 합칠_때도_빈칸이_기존_값을_밀어내지_않는다() {
    let db = db();
    let id = seed(&db, "홍길동", "170315", 3, "가람", 1);

    let row = &rows(&[&["3", "가람", "1", "홍길동", "", "", "", ""]])[0];
    let (input, changes) = db
        .read(|c| {
            let e = load_existing(c, id, 2026)?;
            Ok(merge(&e, row, 2026))
        })
        .unwrap();

    assert!(changes.is_empty());
    assert_eq!(input.mother_phone.as_deref(), Some("010-1234-5678"));
    assert_eq!(input.gender.as_deref(), Some("M"));
    assert_eq!(input.address_raw.as_deref(), Some("○○시 ○○로 1"));
    assert_eq!(input.birth_raw.as_deref(), Some("170315"));
}

// ---------------------------------------------------------------
// 동명이인 · 애매한 중복
// ---------------------------------------------------------------

#[test]
fn 이름이_같아도_생년월일이_다르면_다른_학생이다() {
    let db = db();
    seed(&db, "김민준", "170315", 3, "가람", 1);

    let r = analyze(&db, &[&["4", "나리", "5", "김민준", "남", "160512"]]);
    assert_eq!(r.summary.add, 1, "동명이인은 새 학생으로 넣는다");
    assert_eq!(r.summary.update, 0);
    assert_eq!(r.summary.ambiguous, 0);
}

#[test]
fn 생년월일이_없어_가릴_수_없으면_자동으로_고치지_않는다() {
    let db = db();
    seed(&db, "김민준", "170315", 3, "가람", 1);

    // 엑셀에 생년월일이 없고, 자리도 달라 가릴 수 없다
    let r = analyze(&db, &[&["5", "다솜", "9", "김민준", "남", ""]]);
    assert_eq!(r.summary.ambiguous, 1);
    assert_eq!(r.summary.update, 0);
    assert_eq!(r.summary.add, 0);
    assert!(r.rows[0].reason.as_deref().unwrap().contains("생년월일"));
    assert_eq!(r.rows[0].candidates.len(), 1, "누구와 헷갈리는지 보여 준다");
}

#[test]
fn 생년월일이_없어도_같은_자리_같은_이름이면_같은_학생으로_본다() {
    let db = db();
    seed(&db, "김민준", "170315", 3, "가람", 1);

    let r = analyze(&db, &[&["3", "가람", "1", "김민준", "남", "", "010-7777-8888"]]);
    assert_eq!(r.summary.update, 1, "같은 학년·반·번호에 같은 이름이면 같은 학생");
    assert!(r.rows[0].changes.iter().any(|c| c.field == "부 연락처"));
}

#[test]
fn 같은_자리인데_생년월일이_다르면_중복_의심이다() {
    let db = db();
    seed(&db, "김민준", "170315", 3, "가람", 1);

    // 자리는 같은데 생년월일이 다르다 — 고친 것인지 다른 학생인지 알 수 없다
    let r = analyze(&db, &[&["3", "가람", "1", "김민준", "남", "160512"]]);
    assert_eq!(r.summary.ambiguous, 1);
    assert_eq!(r.summary.update, 0);
    assert!(r.rows[0].reason.as_deref().unwrap().contains("생년월일이 다릅니다"));
}

#[test]
fn 이름과_생년월일이_같은_학생이_이미_여럿이면_손대지_않는다() {
    let db = db();
    seed(&db, "김민준", "170315", 3, "가람", 1);
    seed(&db, "김민준", "170315", 4, "나리", 2);

    let r = analyze(&db, &[&["3", "가람", "1", "김민준", "남", "170315"]]);
    assert_eq!(r.summary.ambiguous, 1);
    assert_eq!(r.rows[0].candidates.len(), 2);
}

// ---------------------------------------------------------------
// 한 파일 안의 중복
// ---------------------------------------------------------------

#[test]
fn 한_파일에_같은_학생이_두_번_있으면_뒤엣것을_중복_의심으로_둔다() {
    let db = db();
    let r = analyze(
        &db,
        &[
            &["3", "가람", "1", "홍길동", "남", "170315"],
            &["3", "가람", "9", "홍길동", "남", "170315"],
        ],
    );
    assert_eq!(r.summary.add, 1);
    assert_eq!(r.summary.ambiguous, 1);
    assert!(r.rows[1].reason.as_deref().unwrap().contains("줄과"));
}

#[test]
fn 한_파일에서_같은_기존_학생을_두_줄이_가리키면_뒤엣것을_뺀다() {
    let db = db();
    seed(&db, "홍길동", "170315", 3, "가람", 1);

    let r = analyze(
        &db,
        &[
            &["3", "가람", "1", "홍길동", "남", "170315", "010-1111-1111"],
            &["3", "가람", "1", "홍길동", "남", "170315", "010-2222-2222"],
        ],
    );
    assert_eq!(r.summary.update, 1);
    assert_eq!(r.summary.ambiguous, 1);
}

#[test]
fn 이름이_같아도_생년월일이_다른_두_줄은_둘_다_신규다() {
    let db = db();
    let r = analyze(
        &db,
        &[
            &["3", "가람", "1", "김민준", "남", "170315"],
            &["4", "나리", "2", "김민준", "남", "160512"],
        ],
    );
    assert_eq!(r.summary.add, 2);
    assert_eq!(r.summary.ambiguous, 0);
}

// ---------------------------------------------------------------
// 지난 학년도에만 있던 학생
// ---------------------------------------------------------------

#[test]
fn 지난_학년도_학생이_올해_명단에_있으면_학적을_새로_만든다() {
    let db = db();
    db.write(|c| settings::create_year(c, 2027)).unwrap();
    seed(&db, "홍길동", "170315", 3, "가람", 1);

    // 2027 학년도로 가져온다
    let rs = rows(&[&["4", "나리", "5", "홍길동", "남", "170315"]]);
    let r = db.read(|c| run(c, &rs, 2027, |_, _| {})).unwrap();

    assert_eq!(r.summary.update, 1, "같은 학생이니 새로 만들지 않는다");
    assert!(
        r.rows[0].changes.iter().any(|c| c.field == "학적"),
        "학적이 새로 생긴다는 것을 보여 준다"
    );
}

// ---------------------------------------------------------------
// 확인 필요 예상
// ---------------------------------------------------------------

#[test]
fn 확인이_필요할_줄을_미리_센다() {
    let db = db();
    let r = analyze(
        &db,
        &[
            // 다 채워진 줄
            &["3", "가람", "1", "홍길동", "남", "170315", "010-1-1", "", "주소"],
            // 성별·번호가 빠진 줄
            &["3", "가람", "", "김영희", "", "170820", "010-2-2", "", "주소"],
        ],
    );
    assert_eq!(r.summary.warned, 1);
    assert!(r.rows[0].warnings.is_empty());
    assert!(!r.rows[1].warnings.is_empty());
}

#[test]
fn 진행_상황을_알려_준다() {
    let db = db();
    let rs = rows(&[
        &["3", "가람", "1", "가", "남", "170315"],
        &["3", "가람", "2", "나", "남", "170316"],
    ]);
    let mut seen: Vec<(usize, usize)> = Vec::new();
    db.read(|c| run(c, &rs, 2026, |done, total| seen.push((done, total))))
        .unwrap();

    assert!(!seen.is_empty());
    assert_eq!(seen.last(), Some(&(2, 2)), "마지막은 다 끝났다고 알린다");
    assert!(seen.iter().all(|(d, t)| d <= t));
}

#[test]
fn 줄이_많아도_알림이_쏟아지지_않는다() {
    // 1,200줄이면 100번 남짓이면 충분하다
    assert!(progress_step(1200) >= 12);
    assert_eq!(progress_step(50), 1, "적을 때는 줄마다 알려도 된다");
}
