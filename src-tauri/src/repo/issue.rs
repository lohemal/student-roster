//! 확인 필요(`issues`) 읽고 쓰기.
//!
//! 규칙 하나: **확인할 것이 있다고 저장을 막지 않는다.** 학생은 일단 등록하고
//! 여기에 표시만 남긴다. 나중에 자료를 고치면 `sync` 가 알아서 닫아 준다.

use rusqlite::{params, Connection};
use serde::Serialize;

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

#[cfg(test)]
#[path = "issue_tests.rs"]
mod issue_tests;
