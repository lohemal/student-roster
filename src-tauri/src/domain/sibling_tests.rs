//! 형제 판정 검사.
//!
//! 가장 위험한 실수는 **빈칸을 일치로 세는 것**이다. 그러면 자료가 덜 채워진
//! 학생끼리 온통 형제가 된다. 경계값을 하나씩 확인한다.
//! 시험에 쓰는 이름·번호는 모두 가상이다.

use super::*;

/// 네 항목을 차례로 넣어 보호자 정보를 만든다. 빈 문자열은 빈칸이다.
fn g(fname: &str, mname: &str, fphone: &str, mphone: &str) -> Guardians {
    let opt = |s: &str| (!s.is_empty()).then(|| s.to_string());
    Guardians {
        father_name: opt(fname),
        mother_name: opt(mname),
        father_phone: opt(fphone),
        mother_phone: opt(mphone),
    }
}

fn matched(a: &Guardians, b: &Guardians) -> Vec<Field> {
    compare(a, b).matched
}

fn conflicts(a: &Guardians, b: &Guardians) -> Vec<Field> {
    compare(a, b).conflicts
}

// ---------------------------------------------------------------
// 일치 개수 경계
// ---------------------------------------------------------------

#[test]
fn 하나도_같지_않으면_후보가_아니다() {
    let a = g("가철수", "가영희", "010-1111-1111", "010-2222-2222");
    let b = g("나철수", "나영희", "010-3333-3333", "010-4444-4444");
    let c = compare(&a, &b);
    assert_eq!(c.matched.len(), 0);
    assert!(!c.is_candidate());
}

#[test]
fn 하나만_같으면_후보가_아니다() {
    let a = g("가철수", "가영희", "010-1111-1111", "010-2222-2222");
    let b = g("가철수", "나영희", "010-3333-3333", "010-4444-4444");
    let c = compare(&a, &b);
    assert_eq!(c.matched, vec![Field::FatherName]);
    assert!(!c.is_candidate(), "한 개만으로는 형제라고 볼 수 없다");
}

#[test]
fn 정확히_둘이_같으면_후보다() {
    let a = g("가철수", "가영희", "010-1111-1111", "010-2222-2222");
    let b = g("가철수", "가영희", "010-3333-3333", "010-4444-4444");
    let c = compare(&a, &b);
    assert_eq!(c.matched, vec![Field::FatherName, Field::MotherName]);
    assert!(c.is_candidate());
}

#[test]
fn 넷_다_같으면_후보다() {
    let a = g("가철수", "가영희", "010-1111-1111", "010-2222-2222");
    let c = compare(&a, &a.clone());
    assert_eq!(c.matched.len(), 4);
    assert!(c.conflicts.is_empty());
    assert!(c.is_candidate());
}

#[test]
fn 셋이_같고_하나가_다르면_후보이면서_불일치도_남는다() {
    // 요구사항 §3 의 예
    let a = g("가철수", "가영희", "010-1111-2222", "010-3333-4444");
    let b = g("가철수", "가영희", "010-9999-8888", "010-3333-4444");
    let c = compare(&a, &b);

    assert_eq!(
        c.matched,
        vec![Field::FatherName, Field::MotherName, Field::MotherPhone]
    );
    assert_eq!(c.conflicts, vec![Field::FatherPhone]);
    assert!(c.is_candidate());
}

// ---------------------------------------------------------------
// 빈칸
// ---------------------------------------------------------------

#[test]
fn 빈칸끼리는_일치가_아니다() {
    // 요구사항 §2 의 예 — 이름 둘만 같고 연락처는 양쪽 다 비었다
    let a = g("가철수", "가영희", "", "");
    let b = g("가철수", "가영희", "", "");
    let c = compare(&a, &b);

    assert_eq!(c.matched.len(), 2, "빈칸 두 개를 더해 4개로 세면 안 된다");
    assert_eq!(c.matched, vec![Field::FatherName, Field::MotherName]);
    assert!(c.conflicts.is_empty(), "빈칸끼리는 불일치도 아니다");
}

#[test]
fn 네_항목이_모두_비면_후보가_아니다() {
    let a = g("", "", "", "");
    let b = g("", "", "", "");
    let c = compare(&a, &b);
    assert!(c.matched.is_empty());
    assert!(c.conflicts.is_empty());
    assert!(!c.is_candidate(), "자료가 없는 학생끼리 형제가 되면 안 된다");
}

#[test]
fn 한쪽만_비면_일치도_불일치도_아니다() {
    let a = g("가철수", "", "", "");
    let b = g("가철수", "가영희", "010-1111-1111", "");
    let c = compare(&a, &b);

    assert_eq!(c.matched, vec![Field::FatherName]);
    assert!(c.conflicts.is_empty(), "한쪽이 비었으면 다르다고 하지 않는다");
    assert!(!c.is_candidate());
}

#[test]
fn 공백만_있는_값은_빈칸으로_본다() {
    let a = g("가철수", "   ", "", "");
    assert!(!a.has(Field::MotherName));
    assert_eq!(a.match_key(Field::MotherName), None);
}

// ---------------------------------------------------------------
// 같은 항목끼리만
// ---------------------------------------------------------------

#[test]
fn 부_연락처와_모_연락처를_엇갈려_견주지_않는다() {
    // A 의 부 연락처 = B 의 모 연락처. 같은 항목끼리 보면 하나도 안 맞는다.
    let a = g("가철수", "가영희", "010-1111-2222", "010-3333-4444");
    let b = g("나철수", "나영희", "010-3333-4444", "010-1111-2222");
    let c = compare(&a, &b);

    assert!(c.matched.is_empty(), "엇갈린 일치를 세면 안 된다: {:?}", c.matched);
    assert!(!c.is_candidate());
}

#[test]
fn 부_성명과_모_성명도_엇갈려_견주지_않는다() {
    let a = g("가철수", "가영희", "", "");
    let b = g("가영희", "가철수", "", "");
    assert!(compare(&a, &b).matched.is_empty());
}

// ---------------------------------------------------------------
// 이름 견주기
// ---------------------------------------------------------------

#[test]
fn 이름의_공백_차이는_같은_것으로_본다() {
    let a = g("김 영희", "", "", "");
    let b = g("김영희", "", "", "");
    assert_eq!(matched(&a, &b), vec![Field::FatherName]);
    assert!(conflicts(&a, &b).is_empty());

    assert_eq!(normalize_name(" 김 영 희 "), "김영희");
}

#[test]
fn 비슷한_이름을_같다고_보지_않는다() {
    let a = g("김영희", "", "", "");
    let b = g("김영이", "", "", "");
    assert!(matched(&a, &b).is_empty(), "한 글자만 달라도 다른 사람이다");
    assert_eq!(conflicts(&a, &b), vec![Field::FatherName]);
}

#[test]
fn 한_글자_이름은_일치로_세지_않는다() {
    // '김' 하나만 적힌 자료로 형제를 묶으면 안 된다
    let a = g("김", "", "", "");
    let b = g("김", "", "", "");
    assert!(matched(&a, &b).is_empty());
}

// ---------------------------------------------------------------
// 연락처 견주기
// ---------------------------------------------------------------

#[test]
fn 연락처는_적는_모양이_달라도_같은_번호로_본다() {
    let a = g("", "", "010-1234-5678", "");
    let b = g("", "", "01012345678", "");
    assert_eq!(matched(&a, &b), vec![Field::FatherPhone]);
    assert!(conflicts(&a, &b).is_empty());
}

#[test]
fn 띄어_쓴_연락처도_같게_본다() {
    let a = g("", "", "010 1234 5678", "");
    let b = g("", "", "010-1234-5678", "");
    assert_eq!(matched(&a, &b), vec![Field::FatherPhone]);
}

#[test]
fn 짧은_번호는_우연히_겹쳐도_일치로_세지_않는다() {
    // 네 자리쯤 되는 값은 여러 학생이 우연히 같을 수 있다
    let a = g("가철수", "", "1234", "");
    let b = g("가철수", "", "1234", "");
    let c = compare(&a, &b);
    assert_eq!(c.matched, vec![Field::FatherName], "{:?}", c.matched);
    assert!(!c.is_candidate(), "짧은 번호로 후보를 만들면 안 된다");
}

#[test]
fn 값이_있는데_다르면_짧은_번호라도_확인_대상이다() {
    let a = g("", "", "1234", "");
    let b = g("", "", "010-1111-2222", "");
    assert_eq!(conflicts(&a, &b), vec![Field::FatherPhone]);
}

// ---------------------------------------------------------------
// 가져올 수 있는 항목
// ---------------------------------------------------------------

#[test]
fn 비어_있는_항목만_가져올_수_있다() {
    // 요구사항 §15 의 예
    let a = g("", "가영희", "", "010-3333-4444");
    let b = g("가철수", "가영희", "010-1111-2222", "010-3333-4444");

    assert_eq!(
        fillable(&a, &b),
        vec![Field::FatherName, Field::FatherPhone],
        "A 에 비어 있는 것만"
    );
    assert!(fillable(&b, &a).is_empty(), "B 는 가져올 것이 없다");
}

#[test]
fn 값이_있으면_다르더라도_가져올_대상이_아니다() {
    let a = g("가철수", "", "010-1111-1111", "");
    let b = g("나철수", "", "010-2222-2222", "");
    assert!(
        fillable(&a, &b).is_empty(),
        "있는 값을 덮어쓰는 일은 없어야 한다"
    );
    assert_eq!(conflicts(&a, &b).len(), 2, "대신 확인 대상이 된다");
}

#[test]
fn 양쪽_다_비어_있으면_가져올_것이_없다() {
    let a = g("", "", "", "");
    let b = g("", "", "", "");
    assert!(fillable(&a, &b).is_empty());
}

// ---------------------------------------------------------------
// 항목 목록 저장
// ---------------------------------------------------------------

#[test]
fn 항목_목록을_글로_담았다_되살린다() {
    let fields = vec![Field::FatherName, Field::MotherPhone];
    let json = to_json(&fields);
    assert!(json.contains("fatherName"), "{json}");
    assert_eq!(from_json(&json), fields);
}

#[test]
fn 이상한_글은_빈_목록으로_읽는다() {
    assert!(from_json("").is_empty());
    assert!(from_json("이상한값").is_empty());
    assert!(from_json("[]").is_empty());
}

#[test]
fn 항목마다_사람이_읽을_이름이_있다() {
    for f in Field::ALL {
        assert!(!f.label().is_empty());
        assert_eq!(Field::parse(f.code()), Some(f));
    }
    assert_eq!(Field::parse("무엇인가"), None);
}

// ---------------------------------------------------------------
// 쌍 찾기 (전수 비교를 피한다)
// ---------------------------------------------------------------

fn entry(id: i64, guardians: Guardians) -> Entry {
    Entry {
        student_id: id,
        guardians,
    }
}

#[test]
fn 값을_나눠_가진_학생끼리만_쌍을_만든다() {
    let entries = vec![
        entry(1, g("가철수", "가영희", "", "")),
        entry(2, g("가철수", "가영희", "", "")),
        entry(3, g("나철수", "나영희", "", "")), // 아무와도 겹치지 않는다
    ];
    let found = find_pairs(&entries);
    assert_eq!(found.pairs, vec![(1, 2)]);
}

#[test]
fn 쌍은_작은_번호가_앞에_오고_한_번만_나온다() {
    let entries = vec![
        entry(7, g("가철수", "가영희", "", "")),
        entry(3, g("가철수", "가영희", "", "")),
    ];
    let found = find_pairs(&entries);
    assert_eq!(found.pairs, vec![(3, 7)], "(7,3) 이나 두 번 나오면 안 된다");
}

#[test]
fn 세_명이면_세_쌍이_나온다() {
    let entries = vec![
        entry(1, g("가철수", "가영희", "", "")),
        entry(2, g("가철수", "가영희", "", "")),
        entry(3, g("가철수", "가영희", "", "")),
    ];
    let found = find_pairs(&entries);
    assert_eq!(found.pairs, vec![(1, 2), (1, 3), (2, 3)]);
}

#[test]
fn 빈칸이_많아도_쌍이_불어나지_않는다() {
    // 자료가 비어 있는 학생 200명. 견줄 쌍이 하나도 없어야 한다.
    let entries: Vec<Entry> = (1..=200).map(|i| entry(i, g("", "", "", ""))).collect();
    let found = find_pairs(&entries);
    assert!(
        found.pairs.is_empty(),
        "빈칸 때문에 {}쌍이 생겼다",
        found.pairs.len()
    );
}

#[test]
fn 너무_흔한_값은_쌍을_만드는_데_쓰지_않는다() {
    // 학교 대표번호를 모두에게 적어 둔 자료 같은 경우
    let entries: Vec<Entry> = (1..=100)
        .map(|i| entry(i, g("", "", "010-0000-0000", "")))
        .collect();
    let found = find_pairs(&entries);
    assert!(found.pairs.is_empty(), "{}쌍이 생겼다", found.pairs.len());
    assert_eq!(found.skipped_values, 1, "몇 건인지는 알려 준다");
}

#[test]
fn 흔한_값이_있어도_다른_항목으로_묶인_쌍은_살아_있다() {
    let mut entries: Vec<Entry> = (1..=100)
        .map(|i| entry(i, g("", "", "010-0000-0000", "")))
        .collect();
    // 이 둘만 부모 이름이 같다
    entries[0].guardians.father_name = Some("가철수".into());
    entries[0].guardians.mother_name = Some("가영희".into());
    entries[1].guardians.father_name = Some("가철수".into());
    entries[1].guardians.mother_name = Some("가영희".into());

    let found = find_pairs(&entries);
    assert!(found.pairs.contains(&(1, 2)), "{:?}", found.pairs);
    assert!(found.pairs.len() < 10, "흔한 값으로 불어나면 안 된다");
}

#[test]
fn 쌍_찾기가_많은_학생에게도_빠르다() {
    // 1,200명. 전수 비교라면 71만 번이다.
    let entries: Vec<Entry> = (1..=1200)
        .map(|i| {
            entry(
                i,
                g(
                    &format!("아버지{}", i / 3), // 세 명씩 같은 부모
                    &format!("어머니{}", i / 3),
                    &format!("010-{:04}-{:04}", i / 3, i / 3),
                    "",
                ),
            )
        })
        .collect();

    let started = std::time::Instant::now();
    let found = find_pairs(&entries);
    let took = started.elapsed();

    // 400 집 × 3쌍 = 1,200 쌍 남짓
    assert!(found.pairs.len() < 2000, "{}쌍", found.pairs.len());
    assert!(took.as_millis() < 500, "{took:?} 걸렸다");
}
