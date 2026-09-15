//! 출력 양식 — 어떤 열을 어떤 파일로.
//!
//! 셋 다 같은 학생 목록(`repo::export::Row`)을 받아 `ExportPlan` 을 만든다. 파일을
//! 쓰는 일은 `export` 모듈 하나가 하므로, **양식이 바뀌면 이 파일만 고치면 된다.**
//!
//! 지키는 것
//!   * 학생 자료를 고쳐 내보내지 않는다. 주소는 원본, 이름은 자르지 않는다.
//!   * 규격에 맞추려고 값을 잘라야 한다면 **시스템이 만든 값**(비고)만 줄인다.
//!     사람이 넣은 값이 규격을 넘으면 자르지 않고 미리 알린다.
//!   * 어느 학생이 왜 빠지는지 파일을 만들기 전에 말한다. 조용히 빼지 않는다.

use serde::{Deserialize, Serialize};

use crate::domain::birth;
use crate::domain::export::{
    self, Column, Grouping, ALIME_MAX_LEN, ALIME_MAX_ROWS,
};
use crate::domain::label;
use crate::repo::export::Row;

use super::{ExportPlan, FilePlan, Problem, SheetPlan, Warning};

/// 어떤 양식으로 내보낼지.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Preset {
    /// 사용자가 열을 고르는 보통 명단
    Custom,
    /// 학교종이 학생명단 업로드 양식
    Schooljongi,
    /// 알림e 문자서비스 업로드 양식
    Alime,
}

impl Preset {
    pub fn label(self) -> &'static str {
        match self {
            Preset::Custom => "사용자 지정 Excel",
            Preset::Schooljongi => "학교종이 학생명단",
            Preset::Alime => "알림e 문자서비스",
        }
    }

    /// 파일 이름 가운데에 들어갈 말.
    fn file_tag(self) -> &'static str {
        match self {
            Preset::Custom => "학생명단",
            Preset::Schooljongi => "학교종이_학생명단",
            Preset::Alime => "알림e_문자명단",
        }
    }
}

// ---------------------------------------------------------------
// 파일 이름 짓기
// ---------------------------------------------------------------

/// `2026학년도_3학년_학생명단` 처럼.
///
/// 조건을 모두 이어 붙이면 이름이 지나치게 길어진다. 학년도와 묶는 단위까지만 넣고
/// 나머지는 사용자가 저장 창에서 고칠 수 있게 둔다.
fn file_name(school_year: i32, group: Option<&str>, preset: Preset, part: Option<usize>) -> String {
    let mut name = format!("{school_year}학년도");
    if let Some(g) = group {
        name.push('_');
        name.push_str(g);
    }
    name.push('_');
    name.push_str(preset.file_tag());
    if let Some(n) = part {
        name.push_str(&format!("_{n}"));
    }
    export::safe_file_name(&name)
}

// ---------------------------------------------------------------
// 묶기
// ---------------------------------------------------------------

/// 파일 하나에 들어갈 학생 묶음.
struct Group<'a> {
    /// 파일 이름에 넣을 말 (`3학년`, `3학년 가람반`). 전체면 None
    tag: Option<String>,
    /// 화면에 보여 줄 이름
    label: String,
    rows: Vec<&'a Row>,
}

/// 학생을 묶는다. **필터로 뽑은 뒤에 묶는다** — 순서가 거꾸로면 학년이 섞인다.
fn split<'a>(rows: &'a [Row], by: Grouping) -> Vec<Group<'a>> {
    match by {
        Grouping::All => vec![Group {
            tag: None,
            label: "전체".into(),
            rows: rows.iter().collect(),
        }],
        Grouping::Grade => {
            let mut grades: Vec<i32> = rows.iter().map(|r| r.grade).collect();
            grades.sort_unstable();
            grades.dedup();
            grades
                .into_iter()
                .map(|g| Group {
                    tag: Some(format!("{g}학년")),
                    label: format!("{g}학년"),
                    rows: rows.iter().filter(|r| r.grade == g).collect(),
                })
                .collect()
        }
        Grouping::GradeClass => {
            // 학생이 이미 명단 차례로 들어오므로 나온 차례를 그대로 쓴다
            let mut keys: Vec<(i32, Option<String>)> = Vec::new();
            for r in rows {
                let key = (r.grade, r.class_name.clone());
                if !keys.contains(&key) {
                    keys.push(key);
                }
            }
            keys.into_iter()
                .map(|(g, cn)| {
                    let label = label::class_label(g, cn.as_deref());
                    let tag = match cn.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                        Some(c) => format!("{g}학년 {c}반"),
                        None => format!("{g}학년 반미정"),
                    };
                    Group {
                        tag: Some(tag),
                        label,
                        rows: rows
                            .iter()
                            .filter(|r| r.grade == g && r.class_name == cn)
                            .collect(),
                    }
                })
                .collect()
        }
    }
}

/// 확인이 필요한 학생을 화면이 알아볼 수 있는 모양으로.
fn problem(r: &Row) -> Problem {
    Problem {
        student_id: r.student_id,
        label: label::student_label(r.grade, r.class_name.as_deref(), &r.name),
    }
}

/// 반이 정해지지 않은 학생. 학급 단위 파일에는 넣을 곳이 없다.
fn without_class(rows: &[Row]) -> Vec<Problem> {
    rows.iter()
        .filter(|r| {
            r.class_name
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .is_none()
        })
        .map(problem)
        .collect()
}

// ---------------------------------------------------------------
// 값 만들기
// ---------------------------------------------------------------

fn text(v: &Option<String>) -> String {
    v.as_deref().unwrap_or("").to_string()
}

/// 생년월일은 화면에 보이는 값과 같게 쓴다.
///
/// 날짜로 읽지 못한 값은 **고치지 않고 원본 그대로** 내보낸다. 임의로 보정하면
/// 틀린 값이 맞는 값처럼 보인다.
fn birth_cell(r: &Row) -> String {
    match r.birth_date.as_deref().and_then(birth::display_of_iso) {
        Some(shown) => shown,
        None => text(&r.birth_raw),
    }
}

fn gender_cell(r: &Row) -> String {
    match r.gender.as_deref() {
        Some("M") => "남".into(),
        Some("F") => "여".into(),
        _ => String::new(),
    }
}

fn cell(r: &Row, col: Column) -> String {
    match col {
        Column::SchoolYear => r.school_year.to_string(),
        Column::Grade => r.grade.to_string(),
        Column::ClassName => text(&r.class_name),
        Column::ClassNo => r.class_no.map(|n| n.to_string()).unwrap_or_default(),
        Column::Name => r.name.clone(),
        Column::Gender => gender_cell(r),
        Column::BirthDate => birth_cell(r),
        // 주소는 **원본** 이다. 정리본은 판정용이라 사용자가 적은 것과 다르다.
        Column::Address => text(&r.address_raw),
        Column::AddressCategory => text(&r.address_category),
        Column::FatherName => text(&r.father_name),
        Column::MotherName => text(&r.mother_name),
        Column::FatherPhone => text(&r.father_phone),
        Column::MotherPhone => text(&r.mother_phone),
        Column::PrimaryPhone => text(&r.primary_phone),
        Column::Sibling => r.siblings.join(", "),
        Column::Note => text(&r.note),
    }
}

// ---------------------------------------------------------------
// 사용자 지정
// ---------------------------------------------------------------

/// 고른 열만, 고른 차례대로.
pub fn custom(rows: &[Row], columns: &[Column], school_year: i32, by: Grouping) -> ExportPlan {
    let headers: Vec<String> = columns.iter().map(|c| c.label().to_string()).collect();
    let widths: Vec<f64> = columns.iter().map(|c| c.width()).collect();

    let files: Vec<FilePlan> = split(rows, by)
        .into_iter()
        .filter(|g| !g.rows.is_empty())
        .map(|g| FilePlan {
            file_name: file_name(school_year, g.tag.as_deref(), Preset::Custom, None),
            label: g.label,
            sheets: vec![SheetPlan {
                name: "학생명단".into(),
                headers: headers.clone(),
                widths: widths.clone(),
                rows: g
                    .rows
                    .iter()
                    .map(|r| columns.iter().map(|c| cell(r, *c)).collect())
                    .collect(),
            }],
        })
        .collect();

    let students = files.iter().map(FilePlan::students).sum();
    ExportPlan {
        files,
        students,
        matched: rows.len(),
        warnings: Vec::new(),
    }
}

// ---------------------------------------------------------------
// 학교종이
// ---------------------------------------------------------------

/// 학교종이 학생명단 — 학급마다 시트 한 장, 한 파일.
///
/// 열은 확인한 양식 그대로 다섯 가지다. 학생휴대폰은 이 프로그램이 다루지 않는
/// 자료이므로 **빈칸으로 둔다** — 없는 값을 지어내지 않는다.
pub fn schooljongi(rows: &[Row], school_year: i32) -> ExportPlan {
    let headers: Vec<String> = ["번호", "이름", "보호자휴대폰1", "보호자휴대폰2", "학생휴대폰"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let widths = vec![6.0, 10.0, 16.0, 16.0, 16.0];

    let no_class = without_class(rows);
    let usable: Vec<&Row> = rows
        .iter()
        .filter(|r| !no_class.iter().any(|p| p.student_id == r.student_id))
        .collect();

    let mut used_names: Vec<String> = Vec::new();
    let mut sheets: Vec<SheetPlan> = Vec::new();

    // 학급 차례는 명단과 같다
    let mut keys: Vec<(i32, Option<String>)> = Vec::new();
    for r in &usable {
        let key = (r.grade, r.class_name.clone());
        if !keys.contains(&key) {
            keys.push(key);
        }
    }
    for (g, cn) in keys {
        let mates: Vec<&&Row> = usable
            .iter()
            .filter(|r| r.grade == g && r.class_name == cn)
            .collect();
        let name = export::safe_sheet_name(
            &export::schooljongi_sheet(g, cn.as_deref()),
            &mut used_names,
        );
        sheets.push(SheetPlan {
            name,
            headers: headers.clone(),
            widths: widths.clone(),
            rows: mates
                .iter()
                .map(|r| {
                    let (p1, p2) = export::guardian_pair(
                        r.primary_phone.as_deref(),
                        r.mother_phone.as_deref(),
                        r.father_phone.as_deref(),
                    );
                    vec![
                        r.class_no.map(|n| n.to_string()).unwrap_or_default(),
                        r.name.clone(),
                        p1.unwrap_or_default(),
                        p2.unwrap_or_default(),
                        // 학생휴대폰 — 이 프로그램에 없는 자료다
                        String::new(),
                    ]
                })
                .collect(),
        });
    }

    let mut warnings = Vec::new();
    if !no_class.is_empty() {
        warnings.push(Warning {
            message: format!(
                "반이 정해지지 않은 학생 {}명은 학급 시트를 만들 수 없어 빠집니다.",
                no_class.len()
            ),
            students: no_class,
            excluded: true,
        });
    }
    // 번호는 학교종이에서 비어 있어도 올릴 수 있다. 막지 않고 알리기만 한다.
    let no_number: Vec<Problem> = usable
        .iter()
        .filter(|r| r.class_no.is_none())
        .map(|r| problem(r))
        .collect();
    if !no_number.is_empty() {
        warnings.push(Warning {
            message: format!(
                "번호가 없는 학생 {}명은 번호 칸이 빈 채로 들어갑니다.",
                no_number.len()
            ),
            students: no_number,
            excluded: false,
        });
    }

    let students = sheets.iter().map(SheetPlan::students).sum();
    let files = if sheets.is_empty() {
        Vec::new()
    } else {
        vec![FilePlan {
            file_name: file_name(school_year, None, Preset::Schooljongi, None),
            label: format!("학급 {}개", sheets.len()),
            sheets,
        }]
    };

    ExportPlan {
        files,
        students,
        matched: rows.len(),
        warnings,
    }
}

// ---------------------------------------------------------------
// 알림e
// ---------------------------------------------------------------

/// 알림e 문자서비스 — 이름·전화번호·비고 세 열.
///
/// **필터 → 묶기 → 1,000명씩 자르기** 차례를 지킨다. 전체를 먼저 1,000명씩 자른 뒤
/// 학년을 섞으면 한 파일에 여러 학년이 들어가 쓸 수 없는 명단이 된다.
pub fn alime(rows: &[Row], school_year: i32, by: Grouping) -> ExportPlan {
    let headers: Vec<String> = ["이름", "전화번호", "비고"].iter().map(|s| s.to_string()).collect();
    let widths = vec![12.0, 16.0, 14.0];

    // 전화번호가 없으면 올릴 수 없다. 빈 줄을 정상 자료처럼 넣지 않는다.
    let mut no_phone: Vec<Problem> = Vec::new();
    let mut too_long: Vec<Problem> = Vec::new();
    let mut usable: Vec<&Row> = Vec::new();

    for r in rows {
        let phone = export::alime_phone(
            r.primary_phone.as_deref(),
            r.mother_phone.as_deref(),
            r.father_phone.as_deref(),
        );
        match phone {
            None => no_phone.push(problem(r)),
            Some(p) => {
                // 사람이 넣은 값이 규격을 넘으면 **자르지 않고** 알린다
                if r.name.chars().count() > ALIME_MAX_LEN
                    || p.chars().count() > ALIME_MAX_LEN
                {
                    too_long.push(problem(r));
                } else {
                    usable.push(r);
                }
            }
        }
    }

    let owned: Vec<Row> = usable.iter().map(|r| (*r).clone()).collect();
    let groups = split(&owned, by);

    let mut files: Vec<FilePlan> = Vec::new();
    for g in groups {
        if g.rows.is_empty() {
            continue;
        }
        // 묶은 **뒤에** 1,000명씩 자른다
        let chunks: Vec<&[&Row]> = g.rows.chunks(ALIME_MAX_ROWS).collect();
        let many = chunks.len() > 1;
        for (i, chunk) in chunks.iter().enumerate() {
            files.push(FilePlan {
                file_name: file_name(
                    school_year,
                    g.tag.as_deref(),
                    Preset::Alime,
                    many.then_some(i + 1),
                ),
                label: if many {
                    format!("{} ({}/{})", g.label, i + 1, chunks.len())
                } else {
                    g.label.clone()
                },
                sheets: vec![SheetPlan {
                    name: "문자명단".into(),
                    headers: headers.clone(),
                    widths: widths.clone(),
                    rows: chunk
                        .iter()
                        .map(|r| {
                            let phone = export::alime_phone(
                                r.primary_phone.as_deref(),
                                r.mother_phone.as_deref(),
                                r.father_phone.as_deref(),
                            )
                            .unwrap_or_default();
                            vec![
                                r.name.clone(),
                                phone,
                                export::alime_note(r.grade, r.class_name.as_deref(), r.class_no),
                            ]
                        })
                        .collect(),
                }],
            });
        }
    }

    let mut warnings = Vec::new();
    if !no_phone.is_empty() {
        warnings.push(Warning {
            message: format!(
                "전화번호가 없는 학생 {}명은 문자명단에 넣을 수 없어 빠집니다. 주보호자·모·부 연락처가 모두 비어 있습니다.",
                no_phone.len()
            ),
            students: no_phone,
            excluded: true,
        });
    }
    if !too_long.is_empty() {
        warnings.push(Warning {
            message: format!(
                "이름이나 전화번호가 {ALIME_MAX_LEN}자를 넘는 학생 {}명은 빠집니다. 값을 줄여 주세요 — 프로그램이 임의로 자르지 않습니다.",
                too_long.len()
            ),
            students: too_long,
            excluded: true,
        });
    }
    if by == Grouping::GradeClass {
        let no_class = without_class(&owned);
        if !no_class.is_empty() {
            warnings.push(Warning {
                message: format!(
                    "반이 정해지지 않아 학년·반별 파일을 만들 수 없는 학생이 {}명 있습니다. '{}' 파일로 따로 나옵니다.",
                    no_class.len(),
                    "반미정"
                ),
                students: no_class,
                excluded: false,
            });
        }
    }

    let students = files.iter().map(FilePlan::students).sum();
    ExportPlan {
        files,
        students,
        matched: rows.len(),
        warnings,
    }
}
