//! 내보내기 규칙 검사. 이름·연락처는 모두 가상이다.

use super::*;

// ---------------------------------------------------------------
// 열
// ---------------------------------------------------------------

#[test]
fn 열_이름과_값이_서로_맞는다() {
    for c in Column::ALL {
        assert_eq!(Column::parse(c.key()), Some(c), "{}", c.key());
        assert!(!c.label().is_empty());
        assert!(c.width() > 0.0);
    }
    assert_eq!(Column::parse("없는열"), None);
}

#[test]
fn 열_이름이_겹치지_않는다() {
    let mut keys: Vec<&str> = Column::ALL.iter().map(|c| c.key()).collect();
    let before = keys.len();
    keys.sort_unstable();
    keys.dedup();
    assert_eq!(keys.len(), before);
}

#[test]
fn 학적_칸만_개인정보가_아니다() {
    assert!(!Column::Grade.is_personal());
    assert!(!Column::ClassNo.is_personal());
    assert!(Column::Name.is_personal());
    assert!(Column::PrimaryPhone.is_personal());
    assert!(Column::Address.is_personal());
}

// ---------------------------------------------------------------
// 시트 이름
// ---------------------------------------------------------------

#[test]
fn 시트_이름은_서른한자를_넘지_않는다() {
    let mut used = Vec::new();
    let long = "아".repeat(50);
    let name = safe_sheet_name(&long, &mut used);
    assert_eq!(name.chars().count(), 31);
}

#[test]
fn 시트_이름에서_쓸_수_없는_글자를_없앤다() {
    let mut used = Vec::new();
    let name = safe_sheet_name("3학년 [2]반:*?/\\", &mut used);
    for bad in ['[', ']', ':', '*', '?', '/', '\\'] {
        assert!(!name.contains(bad), "{bad} 가 남아 있다");
    }
}

#[test]
fn 시트_이름이_겹치면_번호를_붙인다() {
    let mut used = Vec::new();
    assert_eq!(safe_sheet_name("1학년 가람반", &mut used), "1학년 가람반");
    assert_eq!(safe_sheet_name("1학년 가람반", &mut used), "1학년 가람반 (2)");
    assert_eq!(safe_sheet_name("1학년 가람반", &mut used), "1학년 가람반 (3)");
}

#[test]
fn 긴_이름이_겹쳐도_서른한자를_지킨다() {
    let mut used = Vec::new();
    let long = "나".repeat(40);
    let a = safe_sheet_name(&long, &mut used);
    let b = safe_sheet_name(&long, &mut used);
    assert_ne!(a, b);
    assert!(b.chars().count() <= 31, "{} 자", b.chars().count());
}

#[test]
fn 시트_이름이_비면_기본값을_쓴다() {
    let mut used = Vec::new();
    assert_eq!(safe_sheet_name("   ", &mut used), "시트");
}

#[test]
fn 학교종이_시트_이름은_숫자반도_이름반도_자연스럽다() {
    assert_eq!(schooljongi_sheet(1, Some("가람")), "1학년 가람반");
    assert_eq!(schooljongi_sheet(3, Some("2")), "3학년 2반");
    assert_eq!(schooljongi_sheet(6, None), "6학년 반미정");
}

// ---------------------------------------------------------------
// 파일 이름
// ---------------------------------------------------------------

#[test]
fn 파일_이름에서_윈도우가_싫어하는_글자를_없앤다() {
    let out = safe_file_name(r#"2026학년도\3/4:반*이름?"<>|"#);
    for bad in ['\\', '/', ':', '*', '?', '"', '<', '>', '|'] {
        assert!(!out.contains(bad), "{bad} 가 남아 있다");
    }
    assert!(out.contains("2026학년도"));
}

#[test]
fn 파일_이름_끝의_점과_공백을_없앤다() {
    assert_eq!(safe_file_name("명단...  "), "명단");
    assert_eq!(safe_file_name("  앞뒤 공백  "), "앞뒤 공백");
}

#[test]
fn 너무_긴_파일_이름은_줄인다() {
    let out = safe_file_name(&"가".repeat(300));
    assert!(out.chars().count() <= 120);
}

#[test]
fn 이름이_비면_기본값을_쓴다() {
    assert_eq!(safe_file_name("///"), "___");
    assert_eq!(safe_file_name("   "), "명단");
}

// 수식처럼 보이는 값(`=1+1`, `@name`)은 글자 칸으로 쓰면 그대로 남는다.
// 그 확인은 만든 파일을 다시 읽는 `export::export_tests` 에서 한다.

// 알림e 비고 칸은 언제나 빈칸이다. 값을 만드는 규칙이 없으므로 여기서 볼 것도 없다 —
// 실제로 빈칸으로 나가는지는 파일을 다시 읽는 `export::export_tests` 에서 확인한다.

// ---------------------------------------------------------------
// 보호자 연락처 고르기
// ---------------------------------------------------------------

#[test]
fn 보호자_연락처는_주보호자부터_쓴다() {
    let (a, b) = guardian_pair(
        Some("010-1000-0001"),
        Some("010-1000-0002"),
        Some("010-1000-0003"),
    );
    assert_eq!(a.as_deref(), Some("010-1000-0001"));
    assert_eq!(b.as_deref(), Some("010-1000-0002"), "1번에 안 쓰인 모 연락처");
}

#[test]
fn 같은_번호를_두_칸에_적지_않는다() {
    // 주보호자가 모 연락처와 같은 번호다 — 흔한 경우
    let (a, b) = guardian_pair(
        Some("010-1000-0002"),
        Some("010-1000-0002"),
        Some("010-1000-0003"),
    );
    assert_eq!(a.as_deref(), Some("010-1000-0002"));
    assert_eq!(b.as_deref(), Some("010-1000-0003"), "부 연락처로 넘어간다");

    // 표시 모양만 달라도 같은 번호로 본다
    let (a, b) = guardian_pair(Some("01010000002"), Some("010-1000-0002"), None);
    assert_eq!(a.as_deref(), Some("01010000002"));
    assert_eq!(b, None, "같은 번호를 두 번 적지 않는다");
}

#[test]
fn 주보호자가_없으면_모_그다음_부를_쓴다() {
    let (a, b) = guardian_pair(None, Some("010-1000-0002"), Some("010-1000-0003"));
    assert_eq!(a.as_deref(), Some("010-1000-0002"));
    assert_eq!(b.as_deref(), Some("010-1000-0003"));

    let (a, b) = guardian_pair(None, None, Some("010-1000-0003"));
    assert_eq!(a.as_deref(), Some("010-1000-0003"));
    assert_eq!(b, None);

    let (a, b) = guardian_pair(None, None, None);
    assert_eq!((a, b), (None, None));
}

#[test]
fn 빈_문자열은_없는_것으로_본다() {
    let (a, b) = guardian_pair(Some("  "), Some(""), Some("010-1000-0003"));
    assert_eq!(a.as_deref(), Some("010-1000-0003"));
    assert_eq!(b, None);
}

// ---------------------------------------------------------------
// 알림e 전화번호
// ---------------------------------------------------------------

#[test]
fn 알림e_전화번호는_주보호자_모_부_차례다() {
    assert_eq!(
        alime_phone(Some("010-1000-0001"), Some("010-1000-0002"), None).as_deref(),
        Some("010-1000-0001")
    );
    assert_eq!(
        alime_phone(None, Some("010-1000-0002"), Some("010-1000-0003")).as_deref(),
        Some("010-1000-0002")
    );
    assert_eq!(
        alime_phone(None, None, Some("010-1000-0003")).as_deref(),
        Some("010-1000-0003")
    );
    assert_eq!(alime_phone(None, None, None), None);
    assert_eq!(alime_phone(Some("   "), None, None), None, "빈칸은 없는 것");
}

// ---------------------------------------------------------------
// 묶기
// ---------------------------------------------------------------

#[test]
fn 묶기_이름이_있다() {
    for g in [Grouping::All, Grouping::Grade, Grouping::GradeClass] {
        assert!(!g.label().is_empty());
    }
}
