//! 확인 필요(`issues`) 읽고 쓰기.
//!
//! 규칙 하나: **확인할 것이 있다고 저장을 막지 않는다.** 학생은 일단 등록하고
//! 여기에 표시만 남긴다. 나중에 자료를 고치면 `sync` 가 알아서 닫아 준다.

use rusqlite::{params, params_from_iter, types::Value, Connection};
use serde::{Deserialize, Serialize};

use crate::domain::label;
use crate::error::AppResult;

/// 확인 필요 종류. DB CHECK 제약과 같은 값을 쓴다.
// 아직 쓰지 않는 갈래가 있다 — 주소는 Phase 3, 형제는 Phase 4, 번호 중복은 Phase 5.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueKind {
    Address,
    Birth,
    GuardianConflict,
    GuardianFill,
    SiblingCandidate,
    Duplicate,
    Missing,
    ClassAssign,
    NumberDup,
    /// 어느 갈래에도 들지 않는 것.
    Other,
}

impl IssueKind {
    pub fn code(self) -> &'static str {
        match self {
            IssueKind::Address => "ADDRESS",
            IssueKind::Birth => "BIRTH",
            IssueKind::GuardianConflict => "GUARDIAN_CONFLICT",
            IssueKind::GuardianFill => "GUARDIAN_FILL",
            IssueKind::SiblingCandidate => "SIBLING_CANDIDATE",
            IssueKind::Duplicate => "DUPLICATE",
            IssueKind::Missing => "MISSING",
            IssueKind::ClassAssign => "CLASS_ASSIGN",
            IssueKind::NumberDup => "NUMBER_DUP",
            IssueKind::Other => "OTHER",
        }
    }

    /// 이 확인 필요를 고치는 곳 — 학생 상세의 어느 칸을 열어 줄지.
    ///
    /// 확인 필요 화면에서 한 줄을 누르면 곧바로 고칠 수 있는 자리로 보낸다.
    /// 종류만 보여 주고 "알아서 찾아 고치세요" 라고 하면 업무 화면이 아니다.
    pub fn tab(code: &str) -> &'static str {
        match code {
            "SIBLING_CANDIDATE" | "GUARDIAN_FILL" | "GUARDIAN_CONFLICT" => "sibling",
            // 나머지는 모두 기본정보에서 고친다 — 반·번호도 그 칸에 있다
            _ => "basic",
        }
    }

    /// 화면에 보여줄 이름. 종류가 늘면 여기 한 줄만 더한다.
    pub fn label(code: &str) -> &'static str {
        match code {
            "ADDRESS" => "주소 확인",
            "BIRTH" => "생년월일 확인",
            "GUARDIAN_CONFLICT" => "보호자 정보 불일치",
            "GUARDIAN_FILL" => "보호자 정보 보완",
            "SIBLING_CANDIDATE" => "형제 후보 확인",
            "DUPLICATE" => "중복 의심",
            "MISSING" => "필수 정보 누락",
            "CLASS_ASSIGN" => "반·번호 미정",
            "NUMBER_DUP" => "번호 중복",
            _ => "기타 확인",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueRow {
    pub id: i64,
    pub kind: String,
    pub kind_label: String,
    pub message: String,
    pub detail: Option<String>,
    pub ref_id: Option<i64>,
    pub created_at: String,
}

/// 열려 있는 확인 필요 하나를 만든다. 이미 같은 것이 열려 있으면 문구만 갱신한다.
pub fn open(
    c: &Connection,
    student_id: i64,
    kind: IssueKind,
    message: &str,
    detail: Option<&str>,
    ref_id: Option<i64>,
) -> AppResult<()> {
    c.execute(
        "INSERT INTO issues(student_id, kind, message, detail, ref_id, status)
         VALUES (?1, ?2, ?3, ?4, ?5, 'OPEN')
         ON CONFLICT(student_id, kind, COALESCE(ref_id, 0)) WHERE status = 'OPEN'
         DO UPDATE SET message = excluded.message, detail = excluded.detail",
        params![student_id, kind.code(), message, detail, ref_id],
    )?;
    Ok(())
}

/// 해당 종류의 열린 항목을 닫는다 (문제가 사라졌을 때).
pub fn close(c: &Connection, student_id: i64, kind: IssueKind) -> AppResult<()> {
    c.execute(
        "UPDATE issues
            SET status = 'RESOLVED', resolved_at = datetime('now','localtime')
          WHERE student_id = ?1 AND kind = ?2 AND status = 'OPEN'",
        params![student_id, kind.code()],
    )?;
    Ok(())
}

/// 이 종류의 열린 항목 가운데 **`keep` 에 없는 것만** 닫는다.
///
/// 형제처럼 상대가 여럿이면 표시도 여럿이다. 상대 하나가 사라졌을 때
/// 나머지까지 닫아 버리지 않으려고 쓴다.
pub fn close_except(
    c: &Connection,
    student_id: i64,
    kind: IssueKind,
    keep: &[i64],
) -> AppResult<()> {
    let mut st = c.prepare_cached(
        "SELECT id, ref_id FROM issues
          WHERE student_id = ?1 AND kind = ?2 AND status = 'OPEN'",
    )?;
    let rows: Vec<(i64, Option<i64>)> = st
        .query_map(params![student_id, kind.code()], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })?
        .collect::<rusqlite::Result<_>>()?;
    drop(st);

    for (id, ref_id) in rows {
        let keep_this = ref_id.map(|r| keep.contains(&r)).unwrap_or(false);
        if !keep_this {
            c.execute(
                "UPDATE issues
                    SET status = 'RESOLVED', resolved_at = datetime('now','localtime')
                  WHERE id = ?1",
                [id],
            )?;
        }
    }
    Ok(())
}

/// 문제가 있으면 열고 없으면 닫는다. 자료를 고치면 표시가 저절로 사라지게 하는 장치.
pub fn set(
    c: &Connection,
    student_id: i64,
    kind: IssueKind,
    problem: Option<(String, Option<String>)>,
) -> AppResult<()> {
    match problem {
        Some((message, detail)) => open(c, student_id, kind, &message, detail.as_deref(), None),
        None => close(c, student_id, kind),
    }
}

/// 한 학생의 열린 확인 필요.
pub fn list_for_student(c: &Connection, student_id: i64) -> AppResult<Vec<IssueRow>> {
    let mut st = c.prepare(
        "SELECT id, kind, message, detail, ref_id, created_at
           FROM issues
          WHERE student_id = ?1 AND status = 'OPEN'
          ORDER BY id",
    )?;
    let rows = st
        .query_map([student_id], |r| {
            let kind: String = r.get(1)?;
            Ok(IssueRow {
                id: r.get(0)?,
                kind_label: IssueKind::label(&kind).to_string(),
                kind,
                message: r.get(2)?,
                detail: r.get(3)?,
                ref_id: r.get(4)?,
                created_at: r.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueCount {
    pub kind: String,
    pub kind_label: String,
    pub count: i64,
}

/// 종류별 열린 개수. 사이드바 뱃지와 '확인 필요' 화면이 함께 쓴다.
///
/// 지금 학년도에 학적이 있는 학생만 센다 — 지난 학년도 학생의 오래된 표시가
/// 올해 업무 화면에 쌓이면 안 된다.
pub fn summary(c: &Connection, school_year: i32) -> AppResult<Vec<IssueCount>> {
    let mut st = c.prepare(
        "SELECT i.kind, COUNT(*)
           FROM issues i
           JOIN enrollments e ON e.student_id = i.student_id AND e.school_year = ?1
          WHERE i.status = 'OPEN'
          GROUP BY i.kind
          ORDER BY COUNT(*) DESC, i.kind",
    )?;
    let rows = st
        .query_map([school_year], |r| {
            let kind: String = r.get(0)?;
            Ok(IssueCount {
                kind_label: IssueKind::label(&kind).to_string(),
                kind,
                count: r.get(1)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

// ---------------------------------------------------------------
// 확인 필요 화면 — 종류를 가리지 않고 한곳에 모아 본다
// ---------------------------------------------------------------

/// 확인 필요 목록을 추리는 조건. 기본은 **현재 학년도 · 미해결**.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueFilter {
    pub school_year: i32,
    pub grade: Option<i32>,
    pub class_name: Option<String>,
    /// 한 종류만 볼 때. 비어 있으면 전부.
    pub kind: Option<String>,
    /// OPEN(기본) / RESOLVED / ALL
    pub status: Option<String>,
    /// 학생 이름이나 `3-나리 홍길동` 같은 표시로 찾기
    pub q: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueListRow {
    pub id: i64,
    pub student_id: i64,
    /// `3-나리 홍길동` — 지금 학적으로 만든다
    pub student_label: String,
    pub class_no: Option<i32>,
    pub kind: String,
    pub kind_label: String,
    /// 무엇을 확인해야 하는지
    pub message: String,
    pub detail: Option<String>,
    pub status: String,
    pub status_label: String,
    /// 학생 상세의 어느 칸을 열지
    pub tab: String,
    pub created_at: String,
    pub resolved_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueListPage {
    pub rows: Vec<IssueListRow>,
    pub total: i64,
}

pub fn status_label(status: &str) -> &'static str {
    match status {
        "OPEN" => "미해결",
        "DISMISSED" => "넘어감",
        _ => "해결됨",
    }
}

/// `3-나리 홍길동` 을 SQL 안에서 만드는 조각. 이름표는 저장하지 않고 볼 때마다 만든다.
const LABEL_SQL: &str = "(e.grade || '-' || \
     CASE WHEN e.class_name IS NULL OR TRIM(e.class_name) = '' THEN '미정' ELSE e.class_name END \
     || ' ' || s.name)";

fn where_of(f: &IssueFilter) -> (String, Vec<Value>) {
    let mut sql: Vec<String> = vec!["e.school_year = ?".into()];
    let mut args: Vec<Value> = vec![Value::Integer(f.school_year as i64)];

    match f.status.as_deref().unwrap_or("OPEN") {
        "ALL" => {}
        "RESOLVED" => sql.push("i.status <> 'OPEN'".into()),
        other => {
            sql.push("i.status = ?".into());
            args.push(Value::Text(other.to_string()));
        }
    }
    if let Some(g) = f.grade {
        sql.push("e.grade = ?".into());
        args.push(Value::Integer(g as i64));
    }
    if let Some(cn) = f
        .class_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        sql.push("e.class_name = ?".into());
        args.push(Value::Text(cn.to_string()));
    }
    if let Some(k) = f.kind.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        sql.push("i.kind = ?".into());
        args.push(Value::Text(k.to_string()));
    }
    if let Some(q) = f.q.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        // 이름으로도, 화면에 보이는 `3-나리 홍길동` 으로도 찾을 수 있게 한다
        sql.push(format!("(s.name LIKE ? OR {LABEL_SQL} LIKE ?)"));
        args.push(Value::Text(format!("%{q}%")));
        args.push(Value::Text(format!("%{q}%")));
    }
    (sql.join(" AND "), args)
}

/// 확인 필요 목록. 그 학년도에 학적이 있는 학생 것만 본다.
pub fn list(
    c: &Connection,
    f: &IssueFilter,
    limit: i64,
    offset: i64,
) -> AppResult<IssueListPage> {
    let (where_sql, args) = where_of(f);
    let from = "FROM issues i
                JOIN enrollments e ON e.student_id = i.student_id
                JOIN students s    ON s.id = i.student_id";

    let total: i64 = c.query_row(
        &format!("SELECT COUNT(*) {from} WHERE {where_sql}"),
        params_from_iter(args.iter()),
        |r| r.get(0),
    )?;

    // 미해결을 먼저, 그다음 종류·학년·반·번호 순 — 같은 학생 것이 흩어지지 않는다
    let sql = format!(
        "SELECT i.id, i.student_id, {LABEL_SQL}, e.class_no,
                i.kind, i.message, i.detail, i.status, i.created_at, i.resolved_at
           {from}
          WHERE {where_sql}
          ORDER BY CASE WHEN i.status = 'OPEN' THEN 0 ELSE 1 END,
                   i.kind, {order}
          LIMIT ?  OFFSET ?",
        order = label::ORDER_BY_ROSTER,
    );
    let mut with_page = args.clone();
    with_page.push(Value::Integer(limit));
    with_page.push(Value::Integer(offset));

    let mut st = c.prepare(&sql)?;
    let rows: Vec<IssueListRow> = st
        .query_map(params_from_iter(with_page.iter()), |r| {
            let kind: String = r.get(4)?;
            let status: String = r.get(7)?;
            Ok(IssueListRow {
                id: r.get(0)?,
                student_id: r.get(1)?,
                student_label: r.get(2)?,
                class_no: r.get(3)?,
                kind_label: IssueKind::label(&kind).to_string(),
                tab: IssueKind::tab(&kind).to_string(),
                kind,
                message: r.get(5)?,
                detail: r.get(6)?,
                status_label: status_label(&status).to_string(),
                status,
                created_at: r.get(8)?,
                resolved_at: r.get(9)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;

    Ok(IssueListPage { rows, total })
}

#[cfg(test)]
#[path = "issue_tests.rs"]
mod issue_tests;
