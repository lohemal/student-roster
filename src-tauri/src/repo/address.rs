//! 주소 분류·규칙 저장소.
//!
//! 판정 규칙 자체는 `domain::address` 에 있다. 여기서는 규칙과 분류를 읽어 와
//! 넘겨주고, 나온 결과를 학생에게 써 넣는 일만 한다.
//!
//! 지키는 것
//!   * **MANUAL 은 건드리지 않는다.** 사용자가 직접 정한 값이 가장 우선이다.
//!   * 원본 주소(`address_raw`)는 여기서도 절대 고치지 않는다.
//!   * 로그에 주소를 남기지 않는다 — 개인정보다.

use chrono::NaiveDate;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::domain::address::{self, CategoryRef, Decision, Parsed, RuleKind, RuleRef};
use crate::domain::enroll;
use crate::error::{AppError, AppResult};

// ---------------------------------------------------------------
// 규칙·분류 읽어 오기
// ---------------------------------------------------------------

/// 판정에 필요한 것을 한 번에 담아 둔다. 학생마다 다시 읽지 않기 위해서다.
pub struct Book {
    pub rules: Vec<RuleRef>,
    pub categories: Vec<CategoryRef>,
}

impl Book {
    pub fn load(c: &Connection) -> AppResult<Self> {
        let mut st = c.prepare_cached(
            "SELECT r.id, r.kind, r.pattern, r.category_id, ac.name
               FROM address_rules r
               JOIN address_categories ac ON ac.id = r.category_id
              WHERE r.is_active = 1
              ORDER BY r.id",
        )?;
        let rules: Vec<RuleRef> = st
            .query_map([], |r| {
                let kind: String = r.get(1)?;
                Ok(RuleRef {
                    id: r.get(0)?,
                    kind: RuleKind::parse(&kind).unwrap_or(RuleKind::Contains),
                    pattern: r.get(2)?,
                    category_id: r.get(3)?,
                    category_name: r.get(4)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;

        let mut st = c.prepare_cached("SELECT id, name FROM address_categories ORDER BY id")?;
        let categories: Vec<CategoryRef> = st
            .query_map([], |r| {
                Ok(CategoryRef {
                    id: r.get(0)?,
                    name: r.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;

        Ok(Self { rules, categories })
    }

    pub fn classify(&self, parsed: &Parsed) -> Decision {
        address::classify(parsed, &self.rules, &self.categories)
    }
}

// ---------------------------------------------------------------
// 학생 한 명 판정하기
// ---------------------------------------------------------------

/// 학생의 주소를 다시 판정해 저장한다.
///
/// `MANUAL` 이면 아무것도 하지 않고 `None` 을 돌려준다 — 자동화가 사람 판단을 덮지 않는다.
pub fn reclassify(c: &Connection, student_id: i64, book: &Book) -> AppResult<Option<Decision>> {
    let (raw, source): (Option<String>, String) = c.query_row(
        "SELECT address_raw, address_source FROM students WHERE id = ?1",
        [student_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;

    if source == "MANUAL" {
        return Ok(None);
    }

    let parsed = address::parse(raw.as_deref().unwrap_or(""));
    let decision = book.classify(&parsed);
    write_decision(c, student_id, &parsed, &decision)?;
    Ok(Some(decision))
}

/// 판정 결과를 학생에게 써 넣는다. 원본 주소는 건드리지 않는다.
pub fn write_decision(
    c: &Connection,
    student_id: i64,
    parsed: &Parsed,
    decision: &Decision,
) -> AppResult<()> {
    c.execute(
        "UPDATE students
            SET address_norm = ?2,
                address_road = ?3,
                address_category_id = ?4,
                address_source = ?5,
                address_rule_id = ?6
          WHERE id = ?1",
        params![
            student_id,
            (!parsed.norm.is_empty()).then_some(&parsed.norm),
            parsed.road.as_deref(),
            decision.category_id(),
            decision.source(),
            decision.rule_id(),
        ],
    )?;
    Ok(())
}

/// 사용자가 이 학생만 직접 분류한다. 분류를 지우면 다시 자동 판정한다.
pub fn set_manual(c: &Connection, student_id: i64, category_id: Option<i64>) -> AppResult<()> {
    match category_id {
        Some(id) => {
            let exists: i64 = c.query_row(
                "SELECT COUNT(*) FROM address_categories WHERE id = ?1",
                [id],
                |r| r.get(0),
            )?;
            if exists == 0 {
                return Err(AppError::not_found("주소 분류를 찾을 수 없습니다."));
            }
            c.execute(
                "UPDATE students
                    SET address_category_id = ?2, address_source = 'MANUAL', address_rule_id = NULL
                  WHERE id = ?1",
                params![student_id, id],
            )?;
        }
        // 직접 지정을 풀면 규칙에 맡긴다
        None => {
            c.execute(
                "UPDATE students SET address_source = 'NONE' WHERE id = ?1",
                [student_id],
            )?;
            let book = Book::load(c)?;
            reclassify(c, student_id, &book)?;
        }
    }
    Ok(())
}

/// 지금 저장된 판정을 화면에 보여 줄 모양으로 읽는다.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub category_id: Option<i64>,
    pub category_name: Option<String>,
    pub source: String,
    pub source_label: String,
    pub reason: String,
    pub rule_id: Option<i64>,
    pub rule_text: Option<String>,
    /// 충돌이라면 어떤 규칙들이 부딪혔는지
    pub conflicts: Vec<address::Hit>,
    /// 규칙으로 만들 만한 후보
    pub suggestions: Vec<address::Suggestion>,
}

pub fn source_label(source: &str) -> &'static str {
    match source {
        "MANUAL" => "직접 지정",
        "RULE" => "주소 규칙으로 분류",
        "AUTO" => "주소의 단지명으로 자동 분류",
        "CONFLICT" => "규칙 충돌 — 확인 필요",
        _ => "아직 분류되지 않음",
    }
}

/// 학생 한 명의 주소 판정 상태. Drawer 가 쓴다.
pub fn status(c: &Connection, student_id: i64) -> AppResult<Status> {
    let (raw, source, category_id, category_name, rule_id): (
        Option<String>,
        String,
        Option<i64>,
        Option<String>,
        Option<i64>,
    ) = c.query_row(
        "SELECT s.address_raw, s.address_source, s.address_category_id, ac.name, s.address_rule_id
           FROM students s
           LEFT JOIN address_categories ac ON ac.id = s.address_category_id
          WHERE s.id = ?1",
        [student_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
    )?;

    let parsed = address::parse(raw.as_deref().unwrap_or(""));
    let book = Book::load(c)?;

    // 충돌은 지금 규칙으로 다시 따져야 무엇이 부딪히는지 보여 줄 수 있다
    let conflicts = match book.classify(&parsed) {
        Decision::Conflict { hits } if source != "MANUAL" => hits,
        _ => Vec::new(),
    };

    let rule_text = match rule_id {
        Some(id) => c
            .query_row(
                "SELECT r.kind || ' · ' || r.pattern || ' → ' || ac.name
                   FROM address_rules r
                   JOIN address_categories ac ON ac.id = r.category_id
                  WHERE r.id = ?1",
                [id],
                |r| r.get(0),
            )
            .optional()?,
        None => None,
    };

    let reason = if source == "MANUAL" {
        "사용자가 직접 지정했습니다. 자동 재분류가 바꾸지 않습니다.".to_string()
    } else {
        book.classify(&parsed).reason()
    };

    Ok(Status {
        category_id,
        category_name,
        source_label: source_label(&source).to_string(),
        source,
        reason,
        rule_id,
        rule_text,
        conflicts,
        suggestions: address::suggest(&parsed),
    })
}

// ---------------------------------------------------------------
// 분류 관리
// ---------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryRow {
    pub id: i64,
    pub name: String,
    pub sort_order: i64,
    pub is_builtin: bool,
    /// 그 학년도 재학생 가운데 이 분류인 학생 수
    pub student_count: i64,
    /// 이 분류를 가리키는 규칙 수
    pub rule_count: i64,
}

pub fn list_categories(
    c: &Connection,
    school_year: i32,
    asof: NaiveDate,
) -> AppResult<Vec<CategoryRow>> {
    let active = enroll::active_sql("e.", asof);
    let mut st = c.prepare(&format!(
        "SELECT ac.id, ac.name, ac.sort_order, ac.is_builtin,
                (SELECT COUNT(*) FROM students s
                   JOIN enrollments e ON e.student_id = s.id
                  WHERE s.address_category_id = ac.id
                    AND e.school_year = ?1
                    AND {active}),
                (SELECT COUNT(*) FROM address_rules r WHERE r.category_id = ac.id)
           FROM address_categories ac
          ORDER BY ac.sort_order, ac.id",
    ))?;
    let rows = st
        .query_map([school_year], |r| {
            Ok(CategoryRow {
                id: r.get(0)?,
                name: r.get(1)?,
                sort_order: r.get(2)?,
                is_builtin: r.get::<_, i64>(3)? == 1,
                student_count: r.get(4)?,
                rule_count: r.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

fn check_category_name(name: &str) -> AppResult<String> {
    let n = name.trim();
    if n.is_empty() {
        return Err(AppError::invalid("주소 분류 이름을 입력해 주세요."));
    }
    if n.chars().count() > 30 {
        return Err(AppError::invalid("주소 분류 이름은 30자 이내로 입력해 주세요."));
    }
    Ok(n.to_string())
}

pub fn create_category(c: &Connection, name: &str) -> AppResult<i64> {
    let name = check_category_name(name)?;
    // 새 분류는 기본 분류(주택 900 · 기타 999)보다 앞에 오게 둔다
    c.execute(
        "INSERT INTO address_categories(name, kind, sort_order)
         VALUES (?1, 'COMPLEX', COALESCE((SELECT MAX(sort_order) + 1
                                            FROM address_categories
                                           WHERE is_builtin = 0), 1))",
        [&name],
    )
    .map_err(|e| dup_name(e, &name))?;
    Ok(c.last_insert_rowid())
}

pub fn rename_category(c: &Connection, id: i64, name: &str) -> AppResult<()> {
    let name = check_category_name(name)?;
    let n = c
        .execute(
            "UPDATE address_categories SET name = ?2 WHERE id = ?1",
            params![id, &name],
        )
        .map_err(|e| dup_name(e, &name))?;
    if n == 0 {
        return Err(AppError::not_found("주소 분류를 찾을 수 없습니다."));
    }
    Ok(())
}

fn dup_name(e: rusqlite::Error, name: &str) -> AppError {
    let err = AppError::from(e);
    if err.code == "DUPLICATE" {
        return AppError::new("DUPLICATE", format!("'{name}' 분류가 이미 있습니다."));
    }
    err
}

/// 분류를 지운다. **쓰고 있으면 지우지 않고 어디서 쓰는지 알려 준다.**
pub fn delete_category(c: &Connection, id: i64) -> AppResult<()> {
    let (students, rules): (i64, i64) = c.query_row(
        "SELECT (SELECT COUNT(*) FROM students WHERE address_category_id = ?1),
                (SELECT COUNT(*) FROM address_rules WHERE category_id = ?1)",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;

    if students > 0 || rules > 0 {
        let mut parts: Vec<String> = Vec::new();
        if students > 0 {
            parts.push(format!("학생 {students}명"));
        }
        if rules > 0 {
            parts.push(format!("주소 규칙 {rules}개"));
        }
        return Err(AppError::new(
            "IN_USE",
            format!(
                "{}이(가) 이 분류를 쓰고 있어 지울 수 없습니다. 먼저 다른 분류로 옮겨 주세요.",
                parts.join("과 ")
            ),
        ));
    }

    let n = c.execute("DELETE FROM address_categories WHERE id = ?1", [id])?;
    if n == 0 {
        return Err(AppError::not_found("주소 분류를 찾을 수 없습니다."));
    }
    Ok(())
}

// ---------------------------------------------------------------
// 규칙 관리
// ---------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleRow {
    pub id: i64,
    pub kind: String,
    pub kind_label: String,
    pub pattern: String,
    pub category_id: i64,
    pub category_name: String,
    pub is_active: bool,
    pub note: Option<String>,
    /// 이 규칙으로 분류된 학생 수 (그 학년도 재학생)
    pub applied: i64,
    pub created_at: String,
}

pub fn list_rules(
    c: &Connection,
    school_year: i32,
    asof: NaiveDate,
) -> AppResult<Vec<RuleRow>> {
    let active = enroll::active_sql("e.", asof);
    let mut st = c.prepare(&format!(
        "SELECT r.id, r.kind, r.pattern, r.category_id, ac.name, r.is_active, r.note,
                (SELECT COUNT(*) FROM students s
                   JOIN enrollments e ON e.student_id = s.id
                  WHERE s.address_rule_id = r.id
                    AND e.school_year = ?1
                    AND {active}),
                r.created_at
           FROM address_rules r
           JOIN address_categories ac ON ac.id = r.category_id
          ORDER BY CASE r.kind WHEN 'ROAD' THEN 0 WHEN 'COMPLEX' THEN 1 ELSE 2 END,
                   r.pattern",
    ))?;
    let rows = st
        .query_map([school_year], |r| {
            let kind: String = r.get(1)?;
            Ok(RuleRow {
                id: r.get(0)?,
                kind_label: RuleKind::parse(&kind)
                    .map(|k| k.label())
                    .unwrap_or("기타")
                    .to_string(),
                kind,
                pattern: r.get(2)?,
                category_id: r.get(3)?,
                category_name: r.get(4)?,
                is_active: r.get::<_, i64>(5)? == 1,
                note: r.get(6)?,
                applied: r.get(7)?,
                created_at: r.get(8)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

fn check_rule(kind: &str, pattern: &str) -> AppResult<(RuleKind, String)> {
    let k = RuleKind::parse(kind)
        .ok_or_else(|| AppError::invalid("규칙 종류가 올바르지 않습니다."))?;
    let p = address::normalize_pattern(k, pattern);
    if p.trim().is_empty() {
        return Err(AppError::invalid("규칙에 쓸 주소를 입력해 주세요."));
    }
    if k == RuleKind::Road && address::extract_road(&p).is_none() {
        return Err(AppError::invalid(
            "도로명 규칙에는 '○○로 123' 처럼 도로명과 건물번호가 있어야 합니다.",
        ));
    }
    // 너무 짧은 포함 규칙은 온 학교가 걸린다
    if k == RuleKind::Contains && p.chars().count() < 2 {
        return Err(AppError::invalid(
            "포함 규칙은 두 글자 이상이어야 합니다. 너무 짧으면 엉뚱한 주소까지 걸립니다.",
        ));
    }
    Ok((k, p))
}

pub fn create_rule(
    c: &Connection,
    kind: &str,
    pattern: &str,
    category_id: i64,
    note: Option<&str>,
) -> AppResult<i64> {
    let (k, p) = check_rule(kind, pattern)?;
    let exists: i64 = c.query_row(
        "SELECT COUNT(*) FROM address_categories WHERE id = ?1",
        [category_id],
        |r| r.get(0),
    )?;
    if exists == 0 {
        return Err(AppError::not_found("주소 분류를 찾을 수 없습니다."));
    }

    c.execute(
        "INSERT INTO address_rules(kind, pattern, category_id, note) VALUES (?1,?2,?3,?4)",
        params![k.code(), &p, category_id, note],
    )
    .map_err(|e| {
        let err = AppError::from(e);
        if err.code == "DUPLICATE" {
            AppError::new("DUPLICATE", format!("'{p}' 규칙이 이미 있습니다."))
        } else {
            err
        }
    })?;
    Ok(c.last_insert_rowid())
}

pub fn update_rule(
    c: &Connection,
    id: i64,
    kind: &str,
    pattern: &str,
    category_id: i64,
    is_active: bool,
) -> AppResult<()> {
    let (k, p) = check_rule(kind, pattern)?;
    let n = c.execute(
        "UPDATE address_rules
            SET kind = ?2, pattern = ?3, category_id = ?4, is_active = ?5,
                updated_at = datetime('now','localtime')
          WHERE id = ?1",
        params![id, k.code(), &p, category_id, is_active as i64],
    )?;
    if n == 0 {
        return Err(AppError::not_found("주소 규칙을 찾을 수 없습니다."));
    }
    Ok(())
}

pub fn delete_rule(c: &Connection, id: i64) -> AppResult<()> {
    // 이 규칙으로 분류된 학생은 미분류로 되돌린다. 다음 재적용에서 다시 따진다.
    c.execute(
        "UPDATE students
            SET address_category_id = NULL, address_source = 'NONE', address_rule_id = NULL
          WHERE address_rule_id = ?1 AND address_source <> 'MANUAL'",
        [id],
    )?;
    let n = c.execute("DELETE FROM address_rules WHERE id = ?1", [id])?;
    if n == 0 {
        return Err(AppError::not_found("주소 규칙을 찾을 수 없습니다."));
    }
    Ok(())
}

/// 이 규칙이 그 학년도 학생 몇 명에게 새로 걸리는지 세어 본다. **저장하지 않는다.**
pub fn count_matching(
    c: &Connection,
    kind: &str,
    pattern: &str,
    school_year: i32,
    asof: NaiveDate,
) -> AppResult<Matching> {
    let (k, p) = check_rule(kind, pattern)?;
    let active = enroll::active_sql("e.", asof);

    let mut st = c.prepare(&format!(
        "SELECT s.id, s.address_raw, s.address_source
           FROM students s
           JOIN enrollments e ON e.student_id = s.id
          WHERE e.school_year = ?1 AND {active}
            AND s.address_raw IS NOT NULL",
    ))?;
    let rows: Vec<(i64, String, String)> = st
        .query_map([school_year], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;

    let mut out = Matching::default();
    for (_, raw, source) in rows {
        if !address::matches(k, &p, &address::parse(&raw)) {
            continue;
        }
        out.total += 1;
        match source.as_str() {
            "MANUAL" => out.manual += 1,
            "NONE" | "CONFLICT" => out.unclassified += 1,
            _ => out.already += 1,
        }
    }
    Ok(out)
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Matching {
    /// 이 규칙에 걸리는 학생 전체
    pub total: i64,
    /// 그중 아직 분류되지 않은 학생 — 이 규칙으로 새로 정해질 사람들
    pub unclassified: i64,
    /// 이미 다른 방법으로 분류된 학생
    pub already: i64,
    /// 직접 지정이라 건드리지 않을 학생
    pub manual: i64,
}

// ---------------------------------------------------------------
// 전체 다시 적용
// ---------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReapplyResult {
    pub total: i64,
    /// 규칙으로 분류
    pub by_rule: i64,
    /// 주소의 단지명으로 자동 분류
    pub by_auto: i64,
    /// 직접 지정이라 그대로 둔 학생
    pub kept_manual: i64,
    /// 아직 분류하지 못한 학생
    pub unclassified: i64,
    /// 규칙이 부딪혀 정하지 못한 학생
    pub conflict: i64,
    /// 주소 자체가 없는 학생
    pub no_address: i64,
    /// 이번에 분류가 바뀐 학생 수
    pub changed: i64,
}

/// 그 학년도 재학생 전체의 주소를 다시 판정한다.
///
/// `on_progress(done, total)` 로 진행 상황을 알린다.
pub fn reapply(
    c: &Connection,
    school_year: i32,
    asof: NaiveDate,
    mut on_progress: impl FnMut(usize, usize),
) -> AppResult<ReapplyResult> {
    let active = enroll::active_sql("e.", asof);
    let mut st = c.prepare(&format!(
        "SELECT s.id, s.address_raw, s.address_source, s.address_category_id
           FROM students s
           JOIN enrollments e ON e.student_id = s.id
          WHERE e.school_year = ?1 AND {active}
          ORDER BY s.id",
    ))?;
    let rows: Vec<(i64, Option<String>, String, Option<i64>)> = st
        .query_map([school_year], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })?
        .collect::<rusqlite::Result<_>>()?;

    let book = Book::load(c)?;
    let total = rows.len();
    let step = (total / 100).max(1);
    let mut out = ReapplyResult {
        total: total as i64,
        ..Default::default()
    };

    for (i, (id, raw, source, before)) in rows.iter().enumerate() {
        if source == "MANUAL" {
            out.kept_manual += 1;
        } else {
            let parsed = address::parse(raw.as_deref().unwrap_or(""));
            let decision = book.classify(&parsed);
            write_decision(c, *id, &parsed, &decision)?;

            match &decision {
                Decision::Rule { .. } => out.by_rule += 1,
                Decision::Auto { .. } => out.by_auto += 1,
                Decision::Conflict { .. } => out.conflict += 1,
                Decision::NoAddress => out.no_address += 1,
                Decision::Unclassified => out.unclassified += 1,
            }
            if decision.category_id() != *before {
                out.changed += 1;
            }
        }
        if i % step == 0 {
            on_progress(i, total);
        }
    }
    on_progress(total, total);
    Ok(out)
}

/// 규칙 하나를 그 학년도 학생에게 적용한다. 걸리는 학생만 다시 판정한다.
pub fn apply_rule(
    c: &Connection,
    rule_id: i64,
    school_year: i32,
    asof: NaiveDate,
) -> AppResult<i64> {
    let active = enroll::active_sql("e.", asof);
    let (kind, pattern): (String, String) = c
        .query_row(
            "SELECT kind, pattern FROM address_rules WHERE id = ?1",
            [rule_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("주소 규칙을 찾을 수 없습니다."))?;
    let k = RuleKind::parse(&kind).unwrap_or(RuleKind::Contains);

    let mut st = c.prepare(&format!(
        "SELECT s.id, s.address_raw, s.address_category_id
           FROM students s
           JOIN enrollments e ON e.student_id = s.id
          WHERE e.school_year = ?1 AND {active}
            AND s.address_source <> 'MANUAL'
            AND s.address_raw IS NOT NULL",
    ))?;
    let rows: Vec<(i64, String, Option<i64>)> = st
        .query_map([school_year], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;

    let book = Book::load(c)?;
    let mut changed = 0;
    for (id, raw, before) in rows {
        let parsed = address::parse(&raw);
        if !address::matches(k, &pattern, &parsed) {
            continue;
        }
        let decision = book.classify(&parsed);
        write_decision(c, id, &parsed, &decision)?;
        if decision.category_id() != before {
            changed += 1;
        }
    }
    Ok(changed)
}

#[cfg(test)]
#[path = "address_tests.rs"]
mod address_tests;
