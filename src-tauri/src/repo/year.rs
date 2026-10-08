//! 학년도 삭제 — 영향 범위를 세고, 한 트랜잭션 안에서 지운다.
//!
//! `DELETE FROM school_years` 한 줄이 아니다. 잘못 만든 학년도를 **그 학년도에만 속한
//! 자료와 함께** 되돌리는 일이다.
//!
//! 지우는 것 (모두 그 학년도에만 속한다)
//!   * `enrollments`        그 학년도 학적
//!   * `enrollment_events`  그 학년도에 남은 학적 이동·진급 사건
//!   * `renumber_ops`       그 학년도 번호 재정렬 기록
//!   * `imports`            그 학년도로 들여온 기록
//!   * `year_transitions`   이 학년도를 만들어 낸(또는 이 학년도에서 나간) 전환 기록
//!   * `school_years`       학년도 자체
//!
//! **지우지 않는 것**
//!   * `students`      학생 기본정보. 다른 학년도에 학적이 있으면 그대로 다니고 있고,
//!                     없더라도 지우지 않는다 — 이 기능은 학년도 삭제이지 학생 삭제가
//!                     아니다(설계안 §5.10).
//!   * `graduations`   졸업은 **졸업한 해**(6학년이었던 해)에 달린다. 2026→2027 전환이
//!                     만든 졸업 기록은 2026학년도 것이므로 2027학년도를 지워도 남는다.
//!                     되돌리려면 졸업생 화면의 [졸업 취소] 를 쓴다.
//!   * `sibling_links` 형제 관계는 학년도가 없다. 사람 사이의 관계다.
//!   * 주소·보호자·주소 규칙·분류
//!
//! `issues` 는 지우지 않고 **다시 계산한다.** 확인 필요는 자료에서 나온 결과이지
//! 자료가 아니다. 학적이 사라지면 그 학생의 남은 학년도 기준으로 다시 따져야
//! 정확하다.
//!
//! 전환 기록을 함께 지우는 까닭: `year_transitions` 에 2026→2027 이 남아 있으면
//! 다음 전환이 '이미 실행한 전환' 으로 막힌다. 잘못 만든 학년도를 지우고 **다시
//! 전환하는 것**이 이 기능을 쓰는 가장 흔한 까닭이므로 함께 치운다.

use chrono::NaiveDate;
use rusqlite::{Connection, OptionalExtension};

use crate::domain::year::{self, Facts, Refusal};
use crate::error::{AppError, AppResult};
use crate::repo::student;

// ---------------------------------------------------------------
// 영향 범위
// ---------------------------------------------------------------

/// 학년도를 지우면 무슨 일이 생기는가. 화면이 그대로 보여 준다.
///
/// **실제로 셀 수 있는 숫자만 담는다.** 없는 항목을 0 으로 꾸며 두면 사용자가
/// "이것도 지워지나" 하고 읽게 된다.
#[derive(Debug, Clone)]
pub struct Impact {
    pub year: i32,
    pub deletable: bool,
    /// 막혔으면 그 까닭
    pub refusal_code: Option<String>,
    pub refusal_message: Option<String>,

    // ---- 지워지는 것 ----
    pub enrollments: i64,
    /// 그 학년도에 남은 학적 사건 전체
    pub events: i64,
    /// 그 가운데 전입·전출
    pub moves: i64,
    pub renumber_ops: i64,
    pub imports: i64,
    pub transitions: i64,

    // ---- 다시 계산되는 것 ----
    /// 이 학년도 학생에게 열려 있는 확인 필요
    pub issues: i64,

    // ---- 남는 것 ----
    /// 이 학년도에 학적이 있는 학생 (기본정보는 모두 남는다)
    pub students: i64,
    /// 그 가운데 다른 학년도 학적이 하나도 없는 학생
    pub students_without_other_year: i64,
    /// 다른 학년도의 학적
    pub other_enrollments: i64,
    /// 남는 졸업 기록 전체
    pub graduations: i64,
    /// 그 가운데 **이 학년도를 만든 전환이 함께 만든** 졸업 기록
    pub graduations_from_transition: i64,
    /// 그 졸업 기록이 달린 학년도
    pub graduation_year: Option<i32>,
    pub sibling_links: i64,
}

fn count(c: &Connection, sql: &str, args: &[&dyn rusqlite::ToSql]) -> AppResult<i64> {
    Ok(c.query_row(sql, args, |r| r.get(0))?)
}

/// 판정에 쓸 사실을 읽는다.
fn facts(c: &Connection, year: i32) -> AppResult<Facts> {
    let exists = count(
        c,
        "SELECT COUNT(*) FROM school_years WHERE year = ?1",
        &[&year],
    )? > 0;
    let current: Option<i32> = c
        .query_row(
            "SELECT year FROM school_years WHERE is_current = 1",
            [],
            |r| r.get(0),
        )
        .optional()?;
    let graduations = count(
        c,
        "SELECT COUNT(*) FROM graduations WHERE school_year = ?1",
        &[&year],
    )?;
    let later_with_enrollments: Option<i32> = c
        .query_row(
            "SELECT MIN(school_year) FROM enrollments WHERE school_year > ?1",
            [year],
            |r| r.get(0),
        )
        .optional()?
        .flatten();

    Ok(Facts {
        exists,
        current,
        graduations,
        later_with_enrollments,
    })
}

/// 지우면 무엇이 사라지고 무엇이 남는지 센다. **DB 를 건드리지 않는다.**
pub fn impact(c: &Connection, year: i32) -> AppResult<Impact> {
    let f = facts(c, year)?;
    let refusal = year::check(year, &f).err();

    // 이 학년도를 만들어 낸 전환의 원본 학년도 — 함께 만들어진 졸업 기록을 알리는 데 쓴다
    let from_year: Option<i32> = c
        .query_row(
            "SELECT from_year FROM year_transitions WHERE to_year = ?1 ORDER BY id DESC LIMIT 1",
            [year],
            |r| r.get(0),
        )
        .optional()?;
    let graduations_from_transition = match from_year {
        Some(fy) => count(
            c,
            "SELECT COUNT(*) FROM graduations WHERE school_year = ?1",
            &[&fy],
        )?,
        None => 0,
    };

    Ok(Impact {
        year,
        deletable: refusal.is_none(),
        refusal_code: refusal.as_ref().map(|r| r.code().to_string()),
        refusal_message: refusal.as_ref().map(|r| r.message(year)),

        enrollments: count(
            c,
            "SELECT COUNT(*) FROM enrollments WHERE school_year = ?1",
            &[&year],
        )?,
        events: count(
            c,
            "SELECT COUNT(*) FROM enrollment_events WHERE school_year = ?1",
            &[&year],
        )?,
        moves: count(
            c,
            "SELECT COUNT(*) FROM enrollment_events
              WHERE school_year = ?1 AND kind IN ('TRANSFER_IN','TRANSFER_OUT')",
            &[&year],
        )?,
        renumber_ops: count(
            c,
            "SELECT COUNT(*) FROM renumber_ops WHERE school_year = ?1",
            &[&year],
        )?,
        imports: count(
            c,
            "SELECT COUNT(*) FROM imports WHERE school_year = ?1",
            &[&year],
        )?,
        transitions: count(
            c,
            "SELECT COUNT(*) FROM year_transitions WHERE to_year = ?1 OR from_year = ?1",
            &[&year],
        )?,

        issues: count(
            c,
            "SELECT COUNT(*) FROM issues i
               JOIN enrollments e ON e.student_id = i.student_id AND e.school_year = ?1
              WHERE i.status = 'OPEN'",
            &[&year],
        )?,

        students: count(
            c,
            "SELECT COUNT(DISTINCT student_id) FROM enrollments WHERE school_year = ?1",
            &[&year],
        )?,
        students_without_other_year: count(
            c,
            "SELECT COUNT(*) FROM enrollments e
              WHERE e.school_year = ?1
                AND NOT EXISTS (SELECT 1 FROM enrollments o
                                 WHERE o.student_id = e.student_id AND o.school_year <> ?1)",
            &[&year],
        )?,
        other_enrollments: count(
            c,
            "SELECT COUNT(*) FROM enrollments WHERE school_year <> ?1",
            &[&year],
        )?,
        graduations: count(c, "SELECT COUNT(*) FROM graduations", &[])?,
        graduations_from_transition,
        graduation_year: (graduations_from_transition > 0).then_some(from_year).flatten(),
        sibling_links: count(
            c,
            "SELECT COUNT(*) FROM sibling_links WHERE status = 'CONFIRMED'",
            &[],
        )?,
    })
}

// ---------------------------------------------------------------
// 삭제
// ---------------------------------------------------------------

/// 지운 결과. 화면이 "무엇이 사라졌는지" 를 그대로 말할 수 있게 한다.
#[derive(Debug, Clone, Default)]
pub struct Deleted {
    pub year: i32,
    pub enrollments: i64,
    pub events: i64,
    pub renumber_ops: i64,
    pub imports: i64,
    pub transitions: i64,
    /// 확인 필요를 다시 따진 학생 수
    pub resynced: i64,
}

/// 학년도를 지운다. **`Db::write` 안에서 부른다** — 하나라도 실패하면 전부 되돌아간다.
///
/// 지우기 전에 판정을 **다시** 한다. 미리보기를 본 뒤 그사이 현재 학년도가 바뀌었을
/// 수 있기 때문이다.
pub fn delete(c: &Connection, year: i32, today: NaiveDate) -> AppResult<Deleted> {
    let f = facts(c, year)?;
    if let Err(r) = year::check(year, &f) {
        return Err(refusal_error(&r, year));
    }

    // 지우기 전에 누구의 학적이었는지 적어 둔다 — 지운 뒤에는 찾을 수 없다
    let affected: Vec<i64> = {
        let mut st = c
            .prepare("SELECT student_id FROM enrollments WHERE school_year = ?1 ORDER BY student_id")?;
        let ids = st
            .query_map([year], |r| r.get(0))?
            .collect::<rusqlite::Result<Vec<i64>>>()?;
        ids
    };

    let mut out = Deleted {
        year,
        ..Default::default()
    };
    out.events = c.execute(
        "DELETE FROM enrollment_events WHERE school_year = ?1",
        [year],
    )? as i64;
    out.renumber_ops =
        c.execute("DELETE FROM renumber_ops WHERE school_year = ?1", [year])? as i64;
    out.imports = c.execute("DELETE FROM imports WHERE school_year = ?1", [year])? as i64;
    out.transitions = c.execute(
        "DELETE FROM year_transitions WHERE to_year = ?1 OR from_year = ?1",
        [year],
    )? as i64;
    out.enrollments = c.execute("DELETE FROM enrollments WHERE school_year = ?1", [year])? as i64;
    c.execute("DELETE FROM school_years WHERE year = ?1", [year])?;

    // 확인 필요는 남은 자료로 다시 따진다. 학적이 하나도 남지 않은 학생은 어느
    // 화면에도 나오지 않으므로 그대로 둔다 — 전입 예정을 취소하지 않는 한 생기지
    // 않는 경우이고, 섣불리 지우면 되살릴 근거가 사라진다.
    for id in affected {
        let latest: Option<i32> = c
            .query_row(
                "SELECT MAX(school_year) FROM enrollments WHERE student_id = ?1",
                [id],
                |r| r.get(0),
            )
            .optional()?
            .flatten();
        if let Some(y) = latest {
            student::sync_issues(c, id, y, today)?;
            out.resynced += 1;
        }
    }

    // 개인정보는 남기지 않는다 — 학년도와 건수뿐이다
    log::info!(
        "year {} deleted: {} enrollment(s), {} event(s), {} transition record(s)",
        year,
        out.enrollments,
        out.events,
        out.transitions
    );
    Ok(out)
}

/// 막힌 까닭을 사용자에게 보여 줄 오류로.
pub fn refusal_error(r: &Refusal, year: i32) -> AppError {
    match r {
        Refusal::NotFound => AppError::not_found(r.message(year)),
        _ => AppError::invalid(r.message(year)),
    }
}

#[cfg(test)]
#[path = "year_tests.rs"]
mod year_tests;
