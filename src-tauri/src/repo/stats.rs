//! 인원 집계.
//!
//! 전입 반 배정을 도우려고 시작했지만 **화면 전용이 아니다.** 통계 화면이 같은 함수를
//! 쓰고, 앞으로 내보내기도 같은 조건을 쓴다. 숫자를 세는 규칙이 두 군데 있으면
//! 언젠가 서로 다른 값을 말한다.
//!
//! 지키는 것
//!   * 세는 대상은 언제나 **현재 재학생**(`domain::enroll::ACTIVE_STATUS_SQL`).
//!     전출한 학생은 그 즉시 빠진다. 학생명단과 같은 뜻이라야 숫자가 맞는다.
//!   * 성별이 비어 있는 학생을 남·여 어느 쪽에도 **넣지 않는다.** 따로 센다.
//!     남 + 여 가 합계와 다른데 까닭을 알 수 없는 표를 만들지 않기 위해서다.
//!   * 주소는 **분류됨 / 미분류 / 주소 없음**을 절대 섞지 않는다. '기타' 는 사람이
//!     정한 분류이고 '미분류' 는 아직 정하지 못한 상태다. 다른 이야기다.
//!   * **집계 결과를 저장하지 않는다.** 언제나 지금 자료를 세어 답한다.
//!   * 지난 학년도는 `enrollments` 의 그 해 학적으로 센다. 지금 학년·반이 아니다.

use rusqlite::{types::Value, Connection};
use serde::{Deserialize, Serialize};

use crate::domain::enroll::ACTIVE_STATUS_SQL as ACTIVE;
use crate::domain::label;
use crate::error::AppResult;

// ---------------------------------------------------------------
// 모집단
// ---------------------------------------------------------------

/// 어느 학생을 셀지. **모든 표가 이 하나를 함께 쓴다.**
///
/// 통계 화면의 여러 표가 저마다 다른 모집단을 쓰면, 한 화면 안에서 숫자가 서로 맞지
/// 않게 된다. 앞으로 내보내기도 같은 조건을 그대로 받는다.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatFilter {
    pub school_year: i32,
    pub grade: Option<i32>,
    /// 이 주소 분류인 학생만
    pub address_category_id: Option<i64>,
    /// 주소는 있는데 아직 분류하지 못한 학생만
    #[serde(default)]
    pub address_unclassified: bool,
    /// 주소 자체가 없는 학생만
    #[serde(default)]
    pub address_none: bool,
}

impl StatFilter {
    pub fn year(school_year: i32) -> Self {
        Self {
            school_year,
            ..Default::default()
        }
    }

    /// `WHERE` 뒤에 붙일 조건과 값.
    fn where_parts(&self) -> (String, Vec<Value>) {
        let mut sql = vec![
            "e.school_year = ?".to_string(),
            format!("e.{ACTIVE}"),
        ];
        let mut args = vec![Value::Integer(self.school_year as i64)];

        if let Some(g) = self.grade {
            sql.push("e.grade = ?".into());
            args.push(Value::Integer(g as i64));
        }
        if let Some(cat) = self.address_category_id {
            sql.push("s.address_category_id = ?".into());
            args.push(Value::Integer(cat));
        }
        if self.address_unclassified {
            sql.push(UNCLASSIFIED_SQL.into());
        }
        if self.address_none {
            sql.push(NO_ADDRESS_SQL.into());
        }
        (sql.join(" AND "), args)
    }
}

/// 주소가 아예 없는 학생. 저장할 때 빈 문자열은 NULL 이 되지만 지난 자료를 생각해
/// 공백만 있는 값도 없는 것으로 본다.
pub const NO_ADDRESS_SQL: &str = "(s.address_raw IS NULL OR TRIM(s.address_raw) = '')";

/// 주소는 있는데 아직 분류를 정하지 못한 학생.
pub const UNCLASSIFIED_SQL: &str = "(s.address_category_id IS NULL \
     AND s.address_raw IS NOT NULL AND TRIM(s.address_raw) <> '')";

const FROM: &str = "FROM enrollments e JOIN students s ON s.id = e.student_id";

/// 남/여/미입력을 한 번에 세는 조각. 세 갈래가 겹치지도 빠지지도 않는다.
const GENDER_SUMS: &str = "SUM(CASE WHEN s.gender = 'M' THEN 1 ELSE 0 END),
            SUM(CASE WHEN s.gender = 'F' THEN 1 ELSE 0 END),
            SUM(CASE WHEN s.gender IS NULL OR s.gender NOT IN ('M','F') THEN 1 ELSE 0 END),
            COUNT(*)";

// ---------------------------------------------------------------
// 전체
// ---------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub male: i64,
    pub female: i64,
    /// 성별이 비어 있거나 알 수 없는 학생
    pub unknown: i64,
    pub total: i64,
}

impl Counts {
    /// 남 + 여 + 미입력 이 합계와 같은가. 검사에서 표마다 이것을 본다.
    #[cfg(test)]
    pub fn balanced(&self) -> bool {
        self.male + self.female + self.unknown == self.total
    }
}

/// 고른 조건에 드는 학생 전체.
pub fn totals(c: &Connection, f: &StatFilter) -> AppResult<Counts> {
    let (w, args) = f.where_parts();
    let sql = format!("SELECT {GENDER_SUMS} {FROM} WHERE {w}");
    Ok(c.query_row(&sql, rusqlite::params_from_iter(args.iter()), |r| {
        Ok(Counts {
            male: r.get(0)?,
            female: r.get(1)?,
            unknown: r.get(2)?,
            total: r.get(3)?,
        })
    })?)
}

// ---------------------------------------------------------------
// 학년별
// ---------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GradeRow {
    pub grade: i32,
    #[serde(flatten)]
    pub counts: Counts,
}

/// 학년별 남/여/미입력. **실제로 학생이 있는 학년만** 나온다.
pub fn by_grade(c: &Connection, f: &StatFilter) -> AppResult<Vec<GradeRow>> {
    let (w, args) = f.where_parts();
    let sql = format!(
        "SELECT e.grade, {GENDER_SUMS} {FROM} WHERE {w} GROUP BY e.grade ORDER BY e.grade"
    );
    let mut st = c.prepare(&sql)?;
    let rows = st
        .query_map(rusqlite::params_from_iter(args.iter()), |r| {
            Ok(GradeRow {
                grade: r.get(0)?,
                counts: Counts {
                    male: r.get(1)?,
                    female: r.get(2)?,
                    unknown: r.get(3)?,
                    total: r.get(4)?,
                },
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// 그 학년도에 실제로 학생이 있는 학년. 표의 열을 만드는 데 쓴다.
pub fn grades_present(c: &Connection, f: &StatFilter) -> AppResult<Vec<i32>> {
    let (w, args) = f.where_parts();
    let sql = format!("SELECT DISTINCT e.grade {FROM} WHERE {w} ORDER BY e.grade");
    let mut st = c.prepare(&sql)?;
    let rows = st
        .query_map(rusqlite::params_from_iter(args.iter()), |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

// ---------------------------------------------------------------
// 학년 × 반
// ---------------------------------------------------------------

/// 한 반의 인원.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClassCount {
    pub grade: i32,
    pub class_name: Option<String>,
    /// `3-나리` / 반이 없으면 `3-미정`
    pub class_label: String,
    #[serde(flatten)]
    pub counts: Counts,
}

impl ClassCount {
    fn empty(grade: i32, class_name: Option<String>) -> Self {
        Self {
            class_label: label::class_label(grade, class_name.as_deref()),
            grade,
            class_name,
            counts: Counts::default(),
        }
    }
}

/// 학년·반별 남/여/미입력 인원.
///
/// 반이 정해지지 않은 학생도 **빠뜨리지 않고** `미정` 으로 센다. 빠뜨리면 반별 합계가
/// 전체와 달라져 어느 숫자가 맞는지 알 수 없게 된다.
/// 정렬은 학생명단과 같은 규칙(`label::ORDER_BY_CLASS`)을 쓴다.
pub fn by_class(c: &Connection, f: &StatFilter) -> AppResult<Vec<ClassCount>> {
    let (w, args) = f.where_parts();
    let sql = format!(
        "SELECT e.grade, e.class_name, {GENDER_SUMS} {FROM}
          WHERE {w}
          GROUP BY e.grade, e.class_name
          ORDER BY e.grade, {order}",
        order = label::ORDER_BY_CLASS,
    );
    let mut st = c.prepare(&sql)?;
    let rows = st
        .query_map(rusqlite::params_from_iter(args.iter()), |r| {
            let grade: i32 = r.get(0)?;
            let class_name: Option<String> = r.get(1)?;
            Ok(ClassCount {
                class_label: label::class_label(grade, class_name.as_deref()),
                grade,
                class_name,
                counts: Counts {
                    male: r.get(2)?,
                    female: r.get(3)?,
                    unknown: r.get(4)?,
                    total: r.get(5)?,
                },
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

/// 한 학년의 반별 인원 + 그 학년 합계.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GradeCounts {
    pub grade: i32,
    pub classes: Vec<ClassCount>,
    pub total: ClassCount,
}

/// 전입 반 배정 화면이 쓰는 모양. 그 학년의 반별 인원과 합계를 함께 준다.
///
/// 아직 학생이 한 명도 없는 반은 나오지 않는다. 새 반을 만들고 싶으면 반 이름을
/// 직접 적으면 된다 — 프로그램이 반 목록을 정해 주지 않는다.
pub fn grade_counts(c: &Connection, school_year: i32, grade: i32) -> AppResult<GradeCounts> {
    let f = StatFilter {
        grade: Some(grade),
        ..StatFilter::year(school_year)
    };
    let classes = by_class(c, &f)?;
    let mut total = ClassCount::empty(grade, None);
    total.class_label = format!("{grade}학년 합계");
    for r in &classes {
        total.counts.male += r.counts.male;
        total.counts.female += r.counts.female;
        total.counts.unknown += r.counts.unknown;
        total.counts.total += r.counts.total;
    }
    Ok(GradeCounts {
        grade,
        classes,
        total,
    })
}

/// 그 반에서 이미 쓰이고 있는 번호.
///
/// 전입 학생에게 줄 번호가 겹치는지 **저장하기 전에** 알려 주는 데 쓴다.
/// 겹친다고 막지는 않는다 — 겹친 채 저장되면 Phase 5 의 '번호 중복' 표시가 켜진다.
pub fn used_numbers(
    c: &Connection,
    school_year: i32,
    grade: i32,
    class_name: Option<&str>,
) -> AppResult<Vec<i32>> {
    let sql = format!(
        "SELECT DISTINCT class_no FROM enrollments
          WHERE school_year = ?1 AND grade = ?2
            AND ((class_name IS NULL AND ?3 IS NULL) OR class_name = ?3)
            AND class_no IS NOT NULL
            AND {ACTIVE}
          ORDER BY class_no"
    );
    let mut st = c.prepare(&sql)?;
    let rows = st
        .query_map(rusqlite::params![school_year, grade, class_name], |r| {
            r.get(0)
        })?
        .collect::<rusqlite::Result<Vec<i32>>>()?;
    Ok(rows)
}

// ---------------------------------------------------------------
// 주소
// ---------------------------------------------------------------

/// 주소 표의 한 줄이 무엇을 가리키는지.
///
/// **`기타` 와 `미분류` 는 다른 이야기다.** 기타는 사람이 "기타" 라고 정한 분류이고,
/// 미분류는 아직 아무도 정하지 못한 상태다. 합치면 정리된 자료와 밀린 일이 섞인다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AddressBucket {
    /// 사람이 정했거나 규칙으로 정해진 분류
    Category,
    /// 주소는 있는데 아직 분류하지 못했다
    Unclassified,
    /// 주소 자체가 없다
    NoAddress,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddressRow {
    pub bucket: AddressBucket,
    /// 분류일 때만 값이 있다. 학생명단으로 넘어갈 때 쓴다.
    pub category_id: Option<i64>,
    pub name: String,
    /// 학년별 인원. `AddressTable::grades` 와 같은 차례다.
    pub by_grade: Vec<i64>,
    pub total: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddressTable {
    /// 열 머리 — 실제로 학생이 있는 학년만
    pub grades: Vec<i32>,
    pub rows: Vec<AddressRow>,
    /// 열 합계
    pub grade_totals: Vec<i64>,
    pub total: i64,
}

/// 주소 분류 × 학년 교차표.
///
/// 학생이 한 명도 없는 분류도 줄은 보여 준다 — 0명이라는 것도 정보다.
/// 대신 학생이 없는 학년은 열을 만들지 않는다.
pub fn by_address(c: &Connection, f: &StatFilter) -> AppResult<AddressTable> {
    let grades = grades_present(c, f)?;
    let index: std::collections::HashMap<i32, usize> =
        grades.iter().enumerate().map(|(i, g)| (*g, i)).collect();

    // 분류 목록은 사용자가 정한 차례대로
    let mut cats = c.prepare("SELECT id, name FROM address_categories ORDER BY sort_order, id")?;
    let cat_list: Vec<(i64, String)> = cats
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    drop(cats);

    let mut rows: Vec<AddressRow> = cat_list
        .into_iter()
        .map(|(id, name)| AddressRow {
            bucket: AddressBucket::Category,
            category_id: Some(id),
            name,
            by_grade: vec![0; grades.len()],
            total: 0,
        })
        .collect();
    let mut by_id: std::collections::HashMap<i64, usize> = rows
        .iter()
        .enumerate()
        .filter_map(|(i, r)| r.category_id.map(|id| (id, i)))
        .collect();

    let unclassified_at = rows.len();
    rows.push(AddressRow {
        bucket: AddressBucket::Unclassified,
        category_id: None,
        name: "미분류".into(),
        by_grade: vec![0; grades.len()],
        total: 0,
    });
    let none_at = rows.len();
    rows.push(AddressRow {
        bucket: AddressBucket::NoAddress,
        category_id: None,
        name: "주소 없음".into(),
        by_grade: vec![0; grades.len()],
        total: 0,
    });

    let (w, args) = f.where_parts();
    let sql = format!(
        "SELECT s.address_category_id,
                CASE WHEN {NO_ADDRESS_SQL} THEN 1 ELSE 0 END,
                e.grade, COUNT(*)
           {FROM}
          WHERE {w}
          GROUP BY s.address_category_id, 2, e.grade"
    );
    let mut st = c.prepare(&sql)?;
    let cells: Vec<(Option<i64>, i64, i32, i64)> = st
        .query_map(rusqlite::params_from_iter(args.iter()), |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })?
        .collect::<rusqlite::Result<_>>()?;
    drop(st);

    let mut grade_totals = vec![0i64; grades.len()];
    let mut total = 0i64;

    for (cat, no_addr, grade, n) in cells {
        let row_at = match cat {
            Some(id) => match by_id.get(&id) {
                Some(i) => *i,
                None => {
                    // 분류가 지워졌는데 학생에게 남아 있는 경우 — 빠뜨리지 않는다
                    rows.push(AddressRow {
                        bucket: AddressBucket::Category,
                        category_id: Some(id),
                        name: format!("알 수 없는 분류 {id}"),
                        by_grade: vec![0; grades.len()],
                        total: 0,
                    });
                    by_id.insert(id, rows.len() - 1);
                    rows.len() - 1
                }
            },
            None if no_addr == 1 => none_at,
            None => unclassified_at,
        };
        if let Some(col) = index.get(&grade) {
            rows[row_at].by_grade[*col] += n;
            grade_totals[*col] += n;
        }
        rows[row_at].total += n;
        total += n;
    }

    Ok(AddressTable {
        grades,
        rows,
        grade_totals,
        total,
    })
}

/// 주소 판정이 어디까지 되어 있는지. 표 옆에 작게 붙이는 품질 정보다.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddressQuality {
    /// 분류가 정해진 학생
    pub classified: i64,
    /// 주소는 있는데 분류를 정하지 못한 학생
    pub unclassified: i64,
    /// 주소 자체가 없는 학생
    pub no_address: i64,
    /// 사람이 직접 지정
    pub manual: i64,
    /// 사용자 규칙으로 정해짐
    pub rule: i64,
    /// 주소의 단지명으로 저절로 정해짐
    pub auto: i64,
    /// 규칙이 서로 달라 정하지 못함 (미분류에 든다)
    pub conflict: i64,
    pub total: i64,
}

pub fn address_quality(c: &Connection, f: &StatFilter) -> AppResult<AddressQuality> {
    let (w, args) = f.where_parts();
    let sql = format!(
        "SELECT
            SUM(CASE WHEN s.address_category_id IS NOT NULL THEN 1 ELSE 0 END),
            SUM(CASE WHEN {UNCLASSIFIED_SQL} THEN 1 ELSE 0 END),
            SUM(CASE WHEN {NO_ADDRESS_SQL} THEN 1 ELSE 0 END),
            SUM(CASE WHEN s.address_source = 'MANUAL' THEN 1 ELSE 0 END),
            SUM(CASE WHEN s.address_source = 'RULE' THEN 1 ELSE 0 END),
            SUM(CASE WHEN s.address_source = 'AUTO' THEN 1 ELSE 0 END),
            SUM(CASE WHEN s.address_source = 'CONFLICT' THEN 1 ELSE 0 END),
            COUNT(*)
         {FROM} WHERE {w}"
    );
    Ok(c.query_row(&sql, rusqlite::params_from_iter(args.iter()), |r| {
        Ok(AddressQuality {
            classified: r.get(0)?,
            unclassified: r.get(1)?,
            no_address: r.get(2)?,
            manual: r.get(3)?,
            rule: r.get(4)?,
            auto: r.get(5)?,
            conflict: r.get(6)?,
            total: r.get(7)?,
        })
    })?)
}

// ---------------------------------------------------------------
// 정합성
// ---------------------------------------------------------------

/// 같은 모집단을 본 표들의 합계.
///
/// 하나라도 어긋나면 어느 숫자를 믿어야 할지 알 수 없으므로, 화면에 내보이기 전에
/// 스스로 견줘 본다. 검사에서도 이 함수 하나로 모든 표를 한꺼번에 본다.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Consistency {
    pub total: i64,
    /// 남 + 여 + 미입력
    pub gender_sum: i64,
    /// 학년별 합계의 총합
    pub grade_sum: i64,
    /// 반별(미정 포함) 합계의 총합
    pub class_sum: i64,
    /// 주소 상태별 합계의 총합
    pub address_sum: i64,
    /// 주소 × 학년 교차표 모든 칸의 합
    pub cross_sum: i64,
    pub ok: bool,
}

pub fn check_consistency(c: &Connection, f: &StatFilter) -> AppResult<Consistency> {
    let t = totals(c, f)?;
    let grade_sum: i64 = by_grade(c, f)?.iter().map(|r| r.counts.total).sum();
    let class_sum: i64 = by_class(c, f)?.iter().map(|r| r.counts.total).sum();
    let addr = by_address(c, f)?;
    let address_sum: i64 = addr.rows.iter().map(|r| r.total).sum();
    let cross_sum: i64 = addr
        .rows
        .iter()
        .flat_map(|r| r.by_grade.iter())
        .sum::<i64>();

    let gender_sum = t.male + t.female + t.unknown;
    let ok = [gender_sum, grade_sum, class_sum, address_sum, cross_sum]
        .iter()
        .all(|v| *v == t.total);

    Ok(Consistency {
        total: t.total,
        gender_sum,
        grade_sum,
        class_sum,
        address_sum,
        cross_sum,
        ok,
    })
}

#[cfg(test)]
#[path = "stats_tests.rs"]
mod stats_tests;
