//! 내보내기 검사.
//!
//! 계획만 보고 넘어가지 않는다. **실제로 `.xlsx` 를 만든 뒤 calamine 으로 다시 읽어**
//! 셀 값까지 확인한다. 전화번호 앞의 0 이나 수식처럼 보이는 글자는 파일로 나가 봐야
//! 문제가 드러나기 때문이다. 이름·연락처·주소는 모두 가상이다.

use std::path::PathBuf;

use calamine::{Data, Reader, Xlsx};

use super::preset;
use super::*;
use crate::domain::export::{Column, Grouping, ALIME_MAX_ROWS};
use crate::repo::export::Row;

fn tmp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "roster-export-{tag}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 학생 하나. 필요한 것만 채우고 나머지는 빈칸이다.
fn row(id: i64, grade: i32, class_name: Option<&str>, no: Option<i32>, name: &str) -> Row {
    Row {
        student_id: id,
        school_year: 2026,
        grade,
        class_name: class_name.map(str::to_string),
        class_no: no,
        name: name.into(),
        gender: Some("M".into()),
        birth_date: Some("2017-03-15".into()),
        birth_raw: Some("170315".into()),
        address_raw: Some("○○시 가온로 101, 101동 1001호".into()),
        address_category: Some("5단지".into()),
        father_name: Some("남궁바다".into()),
        mother_name: Some("제갈하늘".into()),
        father_phone: Some("010-1000-0003".into()),
        mother_phone: Some("010-1000-0002".into()),
        primary_phone: Some("010-1000-0001".into()),
        note: Some("알레르기 있음".into()),
        siblings: Vec::new(),
    }
}

/// 만든 파일을 다시 읽어 시트별 표로 돌려준다.
fn read_back(path: &std::path::Path) -> Vec<(String, Vec<Vec<String>>)> {
    let mut book: Xlsx<_> = calamine::open_workbook(path).expect("만든 파일을 다시 열지 못했다");
    let names = book.sheet_names().to_vec();
    names
        .into_iter()
        .map(|n| {
            let range = book.worksheet_range(&n).expect("시트를 읽지 못했다");
            let rows: Vec<Vec<String>> = range
                .rows()
                .map(|r| {
                    r.iter()
                        .map(|c| match c {
                            Data::Empty => String::new(),
                            Data::String(s) => s.clone(),
                            other => other.to_string(),
                        })
                        .collect()
                })
                .collect();
            (n, rows)
        })
        .collect()
}

/// 계획을 파일로 쓰고 다시 읽는다 (파일 하나짜리).
fn round_trip(plan: &ExportPlan, tag: &str) -> Vec<(String, Vec<Vec<String>>)> {
    let dir = tmp_dir(tag);
    let path = dir.join(format!("{}.xlsx", plan.files[0].file_name));
    let made = write_plan(plan, &path).unwrap();
    assert_eq!(made.len(), 1);
    read_back(&made[0])
}

// ---------------------------------------------------------------
// 사용자 지정
// ---------------------------------------------------------------

#[test]
fn 고른_열만_고른_차례로_나온다() {
    let rows = vec![row(1, 3, Some("가람"), Some(1), "학생가")];
    let cols = [Column::Name, Column::Grade, Column::ClassNo];
    let plan = preset::custom(&rows, &cols, 2026, Grouping::All);

    let sheets = round_trip(&plan, "columns");
    assert_eq!(sheets.len(), 1);
    let (name, table) = &sheets[0];
    assert_eq!(name, "학생명단");
    assert_eq!(table[0], vec!["이름", "학년", "번호"], "고른 차례 그대로");
    assert_eq!(table[1], vec!["학생가", "3", "1"]);
    assert_eq!(table[1].len(), 3, "고르지 않은 열은 없다");
}

#[test]
fn 전화번호는_앞의_영이_사라지지_않는다() {
    let mut r = row(1, 3, Some("가람"), Some(1), "학생가");
    r.primary_phone = Some("01012345678".into());
    r.father_phone = Some("010-1234-5678".into());
    let plan = preset::custom(
        &[r],
        &[Column::PrimaryPhone, Column::FatherPhone],
        2026,
        Grouping::All,
    );

    let sheets = round_trip(&plan, "phone");
    assert_eq!(
        sheets[0].1[1],
        vec!["01012345678", "010-1234-5678"],
        "숫자로 읽히면 앞의 0 이 사라진다"
    );
}

#[test]
fn 생년월일은_화면과_같은_모양으로_나온다() {
    let r = row(1, 3, Some("가람"), Some(1), "학생가");
    let plan = preset::custom(&[r], &[Column::BirthDate], 2026, Grouping::All);
    assert_eq!(round_trip(&plan, "birth")[0].1[1], vec!["17.03.15."]);
}

#[test]
fn 날짜로_읽지_못한_생년월일은_원본을_그대로_내보낸다() {
    let mut r = row(1, 3, Some("가람"), Some(1), "학생가");
    r.birth_date = None;
    r.birth_raw = Some("확인요망".into());
    let plan = preset::custom(&[r], &[Column::BirthDate], 2026, Grouping::All);
    assert_eq!(
        round_trip(&plan, "badbirth")[0].1[1],
        vec!["확인요망"],
        "임의로 고치면 틀린 값이 맞는 값처럼 보인다"
    );
}

#[test]
fn 주소는_원본을_그대로_내보낸다() {
    let r = row(1, 3, Some("가람"), Some(1), "학생가");
    let plan = preset::custom(
        &[r],
        &[Column::Address, Column::AddressCategory],
        2026,
        Grouping::All,
    );
    let table = round_trip(&plan, "addr");
    assert_eq!(
        table[0].1[1],
        vec!["○○시 가온로 101, 101동 1001호", "5단지"],
        "정리본이 아니라 사용자가 적은 주소"
    );
}

#[test]
fn 본교_형제는_지금_이름표로_나온다() {
    let mut r = row(1, 3, Some("가람"), Some(1), "학생가");
    r.siblings = vec!["1-나리 학생나".into(), "5-가람 학생다".into()];
    let plan = preset::custom(&[r], &[Column::Sibling], 2026, Grouping::All);
    assert_eq!(
        round_trip(&plan, "sib")[0].1[1],
        vec!["1-나리 학생나, 5-가람 학생다"]
    );
}

#[test]
fn 빈_값은_빈칸으로_나온다() {
    let mut r = row(1, 3, None, None, "학생가");
    r.gender = None;
    r.note = None;
    r.address_category = None;
    let plan = preset::custom(
        &[r],
        &[
            Column::Name,
            Column::ClassName,
            Column::ClassNo,
            Column::Gender,
            Column::Note,
            Column::AddressCategory,
        ],
        2026,
        Grouping::All,
    );
    assert_eq!(
        round_trip(&plan, "blank")[0].1[1],
        vec!["학생가", "", "", "", "", ""],
        "없는 값을 임의로 채우지 않는다"
    );
}

#[test]
fn 수식처럼_보이는_값도_글자_그대로_나온다() {
    let mut r = row(1, 3, Some("가람"), Some(1), "=SUM(A1:A9)");
    r.note = Some("-특이사항".into());
    r.address_raw = Some("@우리집".into());
    let plan = preset::custom(
        &[r],
        &[Column::Name, Column::Note, Column::Address],
        2026,
        Grouping::All,
    );
    // 글자 칸으로 쓰므로 Excel 이 계산하지 않고, 값도 고치지 않는다
    assert_eq!(
        round_trip(&plan, "formula")[0].1[1],
        vec!["=SUM(A1:A9)", "-특이사항", "@우리집"]
    );
}

#[test]
fn 학생_차례는_명단과_같다() {
    // 일부러 뒤섞어 넣어도 부르는 쪽(repo)이 정렬해 주므로 그 차례를 지킨다
    let rows = vec![
        row(1, 1, Some("가람"), Some(1), "가학생"),
        row(2, 1, Some("가람"), Some(2), "나학생"),
        row(3, 2, Some("나리"), Some(1), "다학생"),
    ];
    let plan = preset::custom(&rows, &[Column::Name], 2026, Grouping::All);
    let table = round_trip(&plan, "order");
    let names: Vec<String> = table[0].1[1..].iter().map(|r| r[0].clone()).collect();
    assert_eq!(names, vec!["가학생", "나학생", "다학생"]);
}

#[test]
fn 사용자_지정도_학년별로_나눌_수_있다() {
    let rows = vec![
        row(1, 1, Some("가람"), Some(1), "가학생"),
        row(2, 3, Some("나리"), Some(1), "나학생"),
        row(3, 3, Some("나리"), Some(2), "다학생"),
    ];
    let plan = preset::custom(&rows, &[Column::Name], 2026, Grouping::Grade);
    assert_eq!(plan.files.len(), 2);
    assert_eq!(plan.students, 3);
    assert!(plan.files[0].file_name.contains("1학년"));
    assert!(plan.files[1].file_name.contains("3학년"));
}

// ---------------------------------------------------------------
// 학교종이
// ---------------------------------------------------------------

#[test]
fn 학교종이는_학급마다_시트를_만든다() {
    let rows = vec![
        row(1, 1, Some("가람"), Some(1), "가학생"),
        row(2, 1, Some("가람"), Some(2), "나학생"),
        row(3, 1, Some("나리"), Some(1), "다학생"),
        row(4, 3, Some("2"), Some(1), "라학생"),
    ];
    let plan = preset::schooljongi(&rows, 2026);
    assert_eq!(plan.files.len(), 1, "한 파일에 학급 시트를 모은다");

    let sheets = round_trip(&plan, "sj");
    let names: Vec<&str> = sheets.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, vec!["1학년 가람반", "1학년 나리반", "3학년 2반"]);

    let (_, table) = &sheets[0];
    assert_eq!(
        table[0],
        vec!["번호", "이름", "보호자휴대폰1", "보호자휴대폰2", "학생휴대폰"]
    );
    assert_eq!(table.len(), 3, "머리글 + 학생 2명");
    assert_eq!(table[1][0], "1");
    assert_eq!(table[1][1], "가학생");
}

#[test]
fn 학교종이_보호자_연락처는_주보호자부터_쓰고_겹치지_않는다() {
    let mut r = row(1, 1, Some("가람"), Some(1), "가학생");
    r.primary_phone = Some("010-1000-0001".into());
    r.mother_phone = Some("010-1000-0002".into());
    r.father_phone = Some("010-1000-0003".into());

    let mut same = row(2, 1, Some("가람"), Some(2), "나학생");
    same.primary_phone = Some("010-1000-0002".into());
    same.mother_phone = Some("010-1000-0002".into());
    same.father_phone = Some("010-1000-0003".into());

    let plan = preset::schooljongi(&[r, same], 2026);
    let sheets = round_trip(&plan, "sjphone");
    let (_, table) = &sheets[0];

    assert_eq!(&table[1][2..4], &["010-1000-0001", "010-1000-0002"]);
    assert_eq!(
        &table[2][2..4],
        &["010-1000-0002", "010-1000-0003"],
        "주보호자와 모가 같은 번호면 2번은 부로 넘어간다"
    );
}

#[test]
fn 학교종이_학생휴대폰은_빈칸이다() {
    let plan = preset::schooljongi(&[row(1, 1, Some("가람"), Some(1), "가학생")], 2026);
    let sheets = round_trip(&plan, "sjcell");
    assert_eq!(sheets[0].1[1][4], "", "없는 자료를 지어내지 않는다");
}

#[test]
fn 학교종이는_반이_없는_학생을_조용히_빼지_않는다() {
    let rows = vec![
        row(1, 1, Some("가람"), Some(1), "가학생"),
        row(2, 1, None, Some(2), "반없는학생"),
    ];
    let plan = preset::schooljongi(&rows, 2026);

    assert_eq!(plan.matched, 2);
    assert_eq!(plan.students, 1, "반 없는 학생은 넣을 시트가 없다");
    let w = plan
        .warnings
        .iter()
        .find(|w| w.excluded)
        .expect("빠진다는 것을 알려야 한다");
    assert_eq!(w.ids(), vec![2]);
    assert!(w.message.contains("반이 정해지지 않은"));
}

#[test]
fn 학교종이는_번호가_없어도_내보내되_알려_준다() {
    let rows = vec![row(1, 1, Some("가람"), None, "번호없는학생")];
    let plan = preset::schooljongi(&rows, 2026);

    assert_eq!(plan.students, 1, "번호가 없어도 올릴 수 있다");
    let w = plan.warnings.iter().find(|w| !w.excluded).unwrap();
    assert!(w.message.contains("번호가 없는"));

    let sheets = round_trip(&plan, "sjnonum");
    assert_eq!(sheets[0].1[1][0], "", "번호 칸은 빈칸");
}

#[test]
fn 학교종이_시트_이름이_길거나_겹쳐도_안전하다() {
    let long = "아주아주아주아주아주긴반이름입니다정말로";
    let rows = vec![
        row(1, 1, Some(long), Some(1), "가학생"),
        row(2, 1, Some("가/람:반"), Some(1), "나학생"),
    ];
    let plan = preset::schooljongi(&rows, 2026);
    let sheets = round_trip(&plan, "sjname");
    for (name, _) in &sheets {
        assert!(name.chars().count() <= 31, "{name}");
        for bad in ['[', ']', ':', '*', '?', '/', '\\'] {
            assert!(!name.contains(bad), "{name} 에 {bad}");
        }
    }
}

// ---------------------------------------------------------------
// 알림e
// ---------------------------------------------------------------

fn many(n: usize, grade: i32) -> Vec<Row> {
    (0..n)
        .map(|i| {
            row(
                (grade as i64) * 100_000 + i as i64,
                grade,
                Some("가람"),
                Some((i % 40) as i32 + 1),
                &format!("학생{i:04}"),
            )
        })
        .collect()
}

#[test]
fn 알림e는_이름_전화번호_비고_세_열이다() {
    let rows = vec![row(1, 3, Some("나리"), Some(7), "가학생")];
    let plan = preset::alime(&rows, 2026, Grouping::All);
    let sheets = round_trip(&plan, "al");
    let (name, table) = &sheets[0];
    assert_eq!(name, "문자명단");
    assert_eq!(table[0], vec!["이름", "전화번호", "비고"]);
    assert_eq!(table[1], vec!["가학생", "010-1000-0001", "3-나리-7"]);
}

#[test]
fn 알림e_전화번호는_주보호자_모_부_차례다() {
    let mut a = row(1, 3, Some("나리"), Some(1), "가학생");
    a.primary_phone = None;
    let mut b = row(2, 3, Some("나리"), Some(2), "나학생");
    b.primary_phone = None;
    b.mother_phone = None;

    let plan = preset::alime(&[a, b], 2026, Grouping::All);
    let sheets = round_trip(&plan, "alphone");
    assert_eq!(sheets[0].1[1][1], "010-1000-0002", "모 연락처");
    assert_eq!(sheets[0].1[2][1], "010-1000-0003", "부 연락처");
}

#[test]
fn 알림e는_전화번호가_없는_학생을_빈_줄로_넣지_않는다() {
    let mut a = row(1, 3, Some("나리"), Some(1), "연락처없음");
    a.primary_phone = None;
    a.mother_phone = None;
    a.father_phone = None;
    let b = row(2, 3, Some("나리"), Some(2), "정상학생");

    let plan = preset::alime(&[a, b], 2026, Grouping::All);
    assert_eq!(plan.matched, 2);
    assert_eq!(plan.students, 1);

    let w = plan.warnings.iter().find(|w| w.excluded).unwrap();
    assert_eq!(w.ids(), vec![1]);
    assert!(w.message.contains("전화번호가 없는"));

    let sheets = round_trip(&plan, "alnophone");
    assert_eq!(sheets[0].1.len(), 2, "머리글 + 한 명뿐");
}

#[test]
fn 알림e는_열다섯자를_넘는_값을_자르지_않고_알린다() {
    let long = row(1, 3, Some("나리"), Some(1), &"김".repeat(16));
    let ok = row(2, 3, Some("나리"), Some(2), "정상학생");

    let plan = preset::alime(&[long, ok], 2026, Grouping::All);
    assert_eq!(plan.students, 1);
    let w = plan
        .warnings
        .iter()
        .find(|w| w.message.contains("15자"))
        .unwrap();
    assert_eq!(w.ids(), vec![1]);
    assert!(w.excluded);
}

#[test]
fn 알림e_비고는_언제나_열다섯자_안이다() {
    let rows = vec![
        row(1, 3, Some("아주아주아주긴반이름"), Some(7), "가학생"),
        row(2, 6, Some("2"), Some(200), "나학생"),
        row(3, 1, None, None, "다학생"),
    ];
    let plan = preset::alime(&rows, 2026, Grouping::All);
    let sheets = round_trip(&plan, "alnote");
    for line in &sheets[0].1[1..] {
        assert!(line[2].chars().count() <= 15, "{}", line[2]);
    }
}

#[test]
fn 알림e_천명은_한_파일이다() {
    let plan = preset::alime(&many(ALIME_MAX_ROWS, 3), 2026, Grouping::All);
    assert_eq!(plan.files.len(), 1);
    assert_eq!(plan.files[0].students(), 1000);
    assert!(
        !plan.files[0].file_name.ends_with("_1"),
        "한 파일이면 번호를 붙이지 않는다"
    );
}

#[test]
fn 알림e_천한명은_두_파일로_나뉜다() {
    let plan = preset::alime(&many(1001, 3), 2026, Grouping::All);
    assert_eq!(plan.files.len(), 2);
    assert_eq!(plan.files[0].students(), 1000);
    assert_eq!(plan.files[1].students(), 1);
    assert!(plan.files[0].file_name.ends_with("_1"));
    assert!(plan.files[1].file_name.ends_with("_2"));
    assert_eq!(plan.students, 1001);
}

#[test]
fn 알림e_이천명은_두_파일_이천한명은_세_파일이다() {
    assert_eq!(preset::alime(&many(2000, 3), 2026, Grouping::All).files.len(), 2);
    let three = preset::alime(&many(2001, 3), 2026, Grouping::All);
    assert_eq!(three.files.len(), 3);
    assert_eq!(three.files[2].students(), 1);
}

#[test]
fn 알림e_학년별은_학생이_있는_학년만_만든다() {
    let mut rows = many(300, 1);
    rows.extend(many(310, 2));
    rows.extend(many(290, 3));

    let plan = preset::alime(&rows, 2026, Grouping::Grade);
    assert_eq!(plan.files.len(), 3);
    assert_eq!(plan.files[0].students(), 300);
    assert_eq!(plan.files[1].students(), 310);
    assert_eq!(plan.files[2].students(), 290);
    assert!(plan.files[0].file_name.contains("1학년"));
    assert_eq!(plan.students, 900);
}

#[test]
fn 묶은_뒤에_천명씩_자른다() {
    // 학년을 먼저 섞어 자르면 한 파일에 여러 학년이 들어가 쓸 수 없는 명단이 된다
    let mut rows = many(400, 1);
    rows.extend(many(400, 2));
    rows.extend(many(1100, 3));
    rows.extend(many(500, 4));

    let plan = preset::alime(&rows, 2026, Grouping::Grade);
    assert_eq!(plan.files.len(), 5, "1·2·4학년 한 개씩, 3학년 두 개");
    let sizes: Vec<usize> = plan.files.iter().map(FilePlan::students).collect();
    assert_eq!(sizes, vec![400, 400, 1000, 100, 500]);
    assert_eq!(plan.students, 2400);

    // 3학년 파일 두 개에는 3학년만 들어 있다
    assert!(plan.files[2].file_name.contains("3학년"));
    assert!(plan.files[3].file_name.contains("3학년"));
    let dir = tmp_dir("algroup");
    let made = write_plan(&plan, &dir).unwrap();
    assert_eq!(made.len(), 5);
    for (name, table) in read_back(&made[2]) {
        assert_eq!(name, "문자명단");
        for line in &table[1..] {
            assert!(line[2].starts_with("3-"), "{} 는 3학년이 아니다", line[2]);
        }
    }
}

#[test]
fn 알림e_학년반별은_학급마다_파일을_만든다() {
    let rows = vec![
        row(1, 3, Some("가람"), Some(1), "가학생"),
        row(2, 3, Some("나리"), Some(1), "나학생"),
        row(3, 3, Some("나리"), Some(2), "다학생"),
        row(4, 3, Some("2"), Some(1), "라학생"),
    ];
    let plan = preset::alime(&rows, 2026, Grouping::GradeClass);
    assert_eq!(plan.files.len(), 3);
    let names: Vec<&str> = plan.files.iter().map(|f| f.file_name.as_str()).collect();
    assert!(names.iter().any(|n| n.contains("3학년 가람반")));
    assert!(names.iter().any(|n| n.contains("3학년 나리반")));
    assert!(names.iter().any(|n| n.contains("3학년 2반")), "숫자 반도 자연스럽게");
}

#[test]
fn 학년반별에서_반_없는_학생을_다른_반에_섞지_않는다() {
    let rows = vec![
        row(1, 3, Some("가람"), Some(1), "가학생"),
        row(2, 3, None, Some(2), "반없는학생"),
    ];
    let plan = preset::alime(&rows, 2026, Grouping::GradeClass);

    assert_eq!(plan.files.len(), 2, "반미정도 따로 한 파일");
    assert_eq!(plan.students, 2, "빠뜨리지는 않는다");
    let mixed = plan
        .files
        .iter()
        .find(|f| f.file_name.contains("가람"))
        .unwrap();
    assert_eq!(mixed.students(), 1, "가람반 파일에는 가람반 학생만");

    let w = plan
        .warnings
        .iter()
        .find(|w| w.message.contains("반이 정해지지 않아"))
        .expect("알려야 한다");
    assert_eq!(w.ids(), vec![2]);
    assert!(!w.excluded);
}

#[test]
fn 그룹_안에서_천명을_넘으면_그_그룹만_나뉜다() {
    let mut rows = many(10, 1);
    rows.extend(many(1001, 3));

    let plan = preset::alime(&rows, 2026, Grouping::Grade);
    assert_eq!(plan.files.len(), 3);
    let sizes: Vec<usize> = plan.files.iter().map(FilePlan::students).collect();
    assert_eq!(sizes, vec![10, 1000, 1], "1학년은 그대로, 3학년만 둘로");
    assert!(!plan.files[0].file_name.ends_with("_1"), "나뉘지 않은 파일은 번호 없음");
}

// ---------------------------------------------------------------
// 파일 쓰기
// ---------------------------------------------------------------

#[test]
fn 여러_파일은_고른_폴더_안에_만든다() {
    let mut rows = many(3, 1);
    rows.extend(many(2, 2));
    let plan = preset::alime(&rows, 2026, Grouping::Grade);

    let dir = tmp_dir("multi");
    let made = write_plan(&plan, &dir).unwrap();
    assert_eq!(made.len(), 2);
    for p in &made {
        assert!(p.exists(), "{p:?} 가 없다");
        assert_eq!(p.parent().unwrap(), dir);
        assert_eq!(p.extension().unwrap(), "xlsx");
    }
}

#[test]
fn 내보낼_학생이_없으면_파일을_만들지_않는다() {
    let plan = preset::custom(&[], &[Column::Name], 2026, Grouping::All);
    assert!(plan.files.is_empty());
    assert_eq!(plan.students, 0);

    let dir = tmp_dir("empty");
    let err = write_plan(&plan, &dir.join("없음.xlsx")).unwrap_err();
    assert_eq!(err.code, "INVALID_INPUT");
}

#[test]
fn 머리글은_굵게_첫_줄에_있다() {
    let plan = preset::custom(
        &[row(1, 3, Some("가람"), Some(1), "가학생")],
        &[Column::Grade, Column::Name],
        2026,
        Grouping::All,
    );
    let sheets = round_trip(&plan, "head");
    assert_eq!(sheets[0].1[0], vec!["학년", "이름"]);
}

#[test]
fn 천명짜리_파일도_금방_만든다() {
    use std::time::Instant;

    let rows = many(1000, 3);
    let cols = [
        Column::Grade,
        Column::ClassName,
        Column::ClassNo,
        Column::Name,
        Column::Gender,
        Column::BirthDate,
        Column::Address,
        Column::PrimaryPhone,
    ];
    let start = Instant::now();
    let plan = preset::custom(&rows, &cols, 2026, Grouping::All);
    let dir = tmp_dir("speed");
    write_plan(&plan, &dir.join("빠르기.xlsx")).unwrap();
    let elapsed = start.elapsed();

    assert!(
        elapsed.as_millis() < 5000,
        "1,000명 파일을 만드는 데 {elapsed:?} 걸렸다"
    );
    println!("1,000명 8개 열 내보내기: {elapsed:?}");
}
