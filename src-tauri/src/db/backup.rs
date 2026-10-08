//! 백업과 복원.
//!
//! 학생 자료는 이 컴퓨터 안에만 있다. 그래서 **되돌릴 수 있는 것**이 프로그램의 마지막
//! 안전장치다. 이 모듈이 지키는 것:
//!
//!   * **파일을 그대로 복사하지 않는다.** DB 는 WAL 모드라 `.db` 파일만 베끼면 아직
//!     반영되지 않은 내용이 빠진다. SQLite 백업 API(`Db::backup_to`)로 한 시점을 통째로 뜬다.
//!   * **만들었다고 성공이 아니다.** 만든 파일을 다시 열어 `integrity_check` 와 자료 구조를
//!     확인한다. 확인에 실패한 파일은 지우고, 정상 백업을 밀어내지 않는다.
//!   * **복원은 지금 자료를 먼저 지키고 시작한다.** 덮어쓰기 직전에 현재 DB 를 한 번 더
//!     백업하고, 그 백업은 자동 정리 대상이 아니다.
//!   * **열려 있는 DB 파일을 바꿔치기하지 않는다.** 복원할 파일을 옆에 놓아 두었다가
//!     다음 실행 때 연결을 열기 전에 갈아 끼운다. 도중에 멈춰도 지금 자료는 그대로다.
//!   * 로그에 학생 자료를 남기지 않는다. 종류·크기·인원 수만 남긴다.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Local, NaiveDate};
use rusqlite::{Connection, OpenFlags};

use super::{migrate, Db};
use crate::error::{AppError, AppResult};

/// 자동 백업을 몇 개까지 두는가. 하루 한 개이므로 한 달치다.
pub const KEEP_AUTO: usize = 30;

/// 복원할 파일을 잠시 놓아 두는 이름 (`studentroster.db.restore-pending`).
const PENDING_SUFFIX: &str = ".restore-pending";

// ---------------------------------------------------------------
// 종류
// ---------------------------------------------------------------

/// 백업을 왜 만들었는가. **자동 정리는 자동 백업만 건드린다.**
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// 하루 한 번, 앱을 열 때
    Auto,
    /// 사용자가 [지금 백업] 을 눌러서
    Manual,
    /// 복원하기 직전의 자료
    BeforeRestore,
    /// 자료 구조를 바꾸기 직전
    BeforeMigration,
    /// 학년도 전환 직전
    BeforeTransition,
    /// 학년도 삭제 직전
    BeforeYearDelete,
    /// 사용자가 다른 곳에서 가져다 둔 파일
    Other,
}

impl Kind {
    pub fn code(self) -> &'static str {
        match self {
            Kind::Auto => "AUTO",
            Kind::Manual => "MANUAL",
            Kind::BeforeRestore => "BEFORE_RESTORE",
            Kind::BeforeMigration => "BEFORE_MIGRATION",
            Kind::BeforeTransition => "BEFORE_TRANSITION",
            Kind::BeforeYearDelete => "BEFORE_YEAR_DELETE",
            Kind::Other => "OTHER",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Kind::Auto => "자동 백업",
            Kind::Manual => "수동 백업",
            Kind::BeforeRestore => "복원 전",
            Kind::BeforeMigration => "자료 구조 변경 전",
            Kind::BeforeTransition => "학년도 전환 전",
            Kind::BeforeYearDelete => "학년도 삭제 전",
            Kind::Other => "그 밖의 파일",
        }
    }

    /// 파일 이름 앞에 붙는 말.
    fn prefix(self) -> &'static str {
        match self {
            Kind::Auto => "auto_",
            Kind::Manual => "manual_",
            Kind::BeforeRestore => "before_restore_",
            Kind::BeforeMigration => "before_migration_",
            Kind::BeforeTransition => "before_year_transition_",
            Kind::BeforeYearDelete => "before_year_delete_",
            Kind::Other => "",
        }
    }

    /// 자동 정리가 지울 수 있는가. **자동 백업만** 지운다 —
    /// 사람이 만든 것과 큰 작업 직전의 것은 그대로 둔다.
    pub fn cleaned_up(self) -> bool {
        self == Kind::Auto
    }

    /// 파일 이름으로 종류를 가린다.
    pub fn of_file(name: &str) -> Kind {
        for k in [
            Kind::Auto,
            Kind::Manual,
            Kind::BeforeRestore,
            Kind::BeforeMigration,
            Kind::BeforeTransition,
            Kind::BeforeYearDelete,
        ] {
            if name.starts_with(k.prefix()) {
                return k;
            }
        }
        Kind::Other
    }
}

/// `20260915_185000`
pub fn stamp(now: DateTime<Local>) -> String {
    now.format("%Y%m%d_%H%M%S").to_string()
}

/// `manual_20260915_185000.db`
pub fn file_name(kind: Kind, now: DateTime<Local>) -> String {
    format!("{}{}.db", kind.prefix(), stamp(now))
}

/// 파일 이름에 적힌 시각. 읽지 못하면 None.
fn time_of_name(name: &str) -> Option<String> {
    let digits: Vec<&str> = name
        .trim_end_matches(".db")
        .split('_')
        .filter(|p| p.chars().all(|c| c.is_ascii_digit()))
        .collect();
    let date = digits.iter().find(|p| p.len() == 8)?;
    let time = digits.iter().find(|p| p.len() == 6)?;
    Some(format!(
        "{}.{}.{}. {}:{}:{}",
        &date[0..4],
        &date[4..6],
        &date[6..8],
        &time[0..2],
        &time[2..4],
        &time[4..6],
    ))
}

// ---------------------------------------------------------------
// 검사
// ---------------------------------------------------------------

/// 백업 파일을 열어 본 결과.
#[derive(Debug, Clone)]
pub struct Check {
    pub ok: bool,
    /// 자료 구조 버전 (읽지 못했으면 None)
    pub schema_version: Option<i32>,
    /// 학생 수 — 무엇을 되돌리는지 알 수 있게. 개인정보가 아니다.
    pub students: Option<i64>,
    /// 무엇이 문제인지 한 줄
    pub problem: Option<String>,
}

impl Check {
    fn bad(problem: impl Into<String>) -> Self {
        Check {
            ok: false,
            schema_version: None,
            students: None,
            problem: Some(problem.into()),
        }
    }
}

/// 이 파일이 **이 프로그램의 성한 자료**인가.
///
/// 만들고 나서도, 되돌리기 전에도 같은 검사를 지난다. 다른 프로그램의 SQLite 파일을
/// 잘못 골라도 지금 자료를 건드리지 않게 하려는 것이다.
pub fn check(path: &Path) -> Check {
    if !path.exists() {
        return Check::bad("파일을 찾을 수 없습니다.");
    }
    let conn = match Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY) {
        Ok(c) => c,
        Err(e) => return Check::bad(format!("SQLite 파일로 열지 못했습니다. ({e})")),
    };

    // 1) 파일이 성한가
    let integrity: Result<String, _> =
        conn.query_row("PRAGMA integrity_check", [], |r| r.get(0));
    match integrity {
        Ok(v) if v == "ok" => {}
        Ok(v) => return Check::bad(format!("자료가 손상되었습니다. ({v})")),
        Err(e) => return Check::bad(format!("자료를 읽지 못했습니다. ({e})")),
    }

    // 2) 이 프로그램의 자료인가
    let mine: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
              WHERE type = 'table' AND name IN ('students','enrollments','school_years')",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if mine < 3 {
        return Check::bad("이 프로그램의 자료 파일이 아닙니다.");
    }

    // 3) 이 버전이 읽을 수 있는 구조인가
    let version: i32 = match migrate::current_version(&conn) {
        Ok(v) => v,
        Err(e) => return Check::bad(format!("자료 구조 버전을 읽지 못했습니다. ({e})")),
    };
    let students: Option<i64> = conn
        .query_row("SELECT COUNT(*) FROM students", [], |r| r.get(0))
        .ok();

    if version > migrate::latest_version() {
        return Check {
            ok: false,
            schema_version: Some(version),
            students,
            problem: Some(format!(
                "더 최신 버전의 프로그램에서 만든 자료입니다(v{version}). 프로그램을 먼저 업데이트해 주세요."
            )),
        };
    }
    if version < 1 {
        return Check {
            ok: false,
            schema_version: Some(version),
            students,
            problem: Some("자료 구조가 만들어지기 전의 파일입니다.".into()),
        };
    }

    Check {
        ok: true,
        schema_version: Some(version),
        students,
        problem: None,
    }
}

// ---------------------------------------------------------------
// 목록
// ---------------------------------------------------------------

/// 백업 파일 하나.
#[derive(Debug, Clone)]
pub struct Entry {
    pub file_name: String,
    pub path: String,
    pub kind: Kind,
    /// `2026.09.15. 18:50:00`
    pub created_at: String,
    pub size: u64,
    pub check: Check,
}

/// 백업 폴더를 훑는다. **파일마다 열어 보고** 정상인지까지 알려 준다.
pub fn list(dir: &Path) -> AppResult<Vec<Entry>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for item in std::fs::read_dir(dir)? {
        let item = item?;
        let path = item.path();
        if path.extension().and_then(|e| e.to_str()) != Some("db") {
            continue;
        }
        let file_name = item.file_name().to_string_lossy().to_string();
        let meta = item.metadata()?;
        let created_at = time_of_name(&file_name).unwrap_or_else(|| {
            meta.modified()
                .ok()
                .map(|t| DateTime::<Local>::from(t).format("%Y.%m.%d. %H:%M:%S").to_string())
                .unwrap_or_default()
        });
        out.push(Entry {
            kind: Kind::of_file(&file_name),
            check: check(&path),
            path: path.display().to_string(),
            file_name,
            created_at,
            size: meta.len(),
        });
    }
    // 새 것부터 (이름에 시각이 들어 있어 이름 역순이 곧 시각 역순이다)
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(b.file_name.cmp(&a.file_name)));
    Ok(out)
}

// ---------------------------------------------------------------
// 만들기
// ---------------------------------------------------------------

/// 백업을 만들고 **바로 확인한다.** 확인에 실패하면 그 파일을 지우고 오류를 낸다.
pub fn create(db: &Db, kind: Kind, now: DateTime<Local>) -> AppResult<Entry> {
    let dir = db.backup_dir();
    let name = file_name(kind, now);
    let path = dir.join(&name);

    db.backup_to(&path)?;

    let result = check(&path);
    if !result.ok {
        // 성하지 않은 백업은 남겨 두지 않는다 — 나중에 이것을 믿고 복원하면 더 나쁘다
        let _ = std::fs::remove_file(&path);
        return Err(AppError::new(
            "BACKUP_FAILED",
            format!(
                "백업을 만들었지만 확인에 실패해 지웠습니다. {}",
                result.problem.clone().unwrap_or_default()
            ),
        ));
    }

    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    log::info!("backup created: {} ({} bytes)", kind.code(), size);

    Ok(Entry {
        file_name: name.clone(),
        path: path.display().to_string(),
        kind,
        created_at: time_of_name(&name).unwrap_or_default(),
        size,
        check: result,
    })
}

/// 오늘 만들어 둔 **정상** 자동 백업이 있는가.
pub fn has_auto_today(dir: &Path, today: NaiveDate) -> bool {
    let tag = today.format("%Y%m%d").to_string();
    list(dir)
        .unwrap_or_default()
        .iter()
        .any(|e| e.kind == Kind::Auto && e.check.ok && e.file_name.contains(&tag))
}

/// 자동 백업이 무한히 쌓이지 않게 한다. 지운 개수를 돌려준다.
///
/// **자동 백업만** 지운다. 수동 백업과 큰 작업 직전의 백업은 사용자가 지울 때까지 남는다.
pub fn cleanup_auto(dir: &Path, keep: usize) -> AppResult<usize> {
    let autos: Vec<Entry> = list(dir)?
        .into_iter()
        .filter(|e| e.kind.cleaned_up())
        .collect();
    if autos.len() <= keep {
        return Ok(0);
    }
    let mut gone = 0;
    for e in autos.iter().skip(keep) {
        if std::fs::remove_file(&e.path).is_ok() {
            gone += 1;
        }
    }
    if gone > 0 {
        log::info!("backup cleanup: removed {gone} old auto backup(s)");
    }
    Ok(gone)
}

/// 앱을 열 때 하루 한 번. 이미 오늘 것이 있으면 아무것도 하지 않는다.
///
/// **자료가 성할 때만 만든다** — 손상된 DB 로 뜬 백업이 멀쩡한 지난 백업을 밀어내면
/// 되돌릴 곳이 사라진다.
pub fn auto_if_needed(db: &Db, today: NaiveDate, now: DateTime<Local>) -> AppResult<Option<Entry>> {
    if has_auto_today(&db.backup_dir(), today) {
        return Ok(None);
    }
    let entry = create(db, Kind::Auto, now)?;
    cleanup_auto(&db.backup_dir(), KEEP_AUTO)?;
    Ok(Some(entry))
}

// ---------------------------------------------------------------
// 복원
// ---------------------------------------------------------------

fn pending_path(db_path: &Path) -> PathBuf {
    let mut p = db_path.as_os_str().to_os_string();
    p.push(PENDING_SUFFIX);
    PathBuf::from(p)
}

/// 복원할 파일을 **옆에 놓아 둔다.** 지금 자료는 아직 그대로다.
///
/// 열려 있는 DB 파일을 그 자리에서 바꿔치기하지 않는다. 다음 실행 때 연결을 열기 전에
/// `apply_pending` 이 갈아 끼운다 — 도중에 멈춰도 지금 자료가 남아 있는 까닭이다.
pub fn stage_restore(db_path: &Path, source: &Path) -> AppResult<Check> {
    let result = check(source);
    if !result.ok {
        return Err(AppError::invalid(format!(
            "이 파일로는 되돌릴 수 없습니다. {}",
            result.problem.clone().unwrap_or_default()
        )));
    }

    let pending = pending_path(db_path);
    // 옆에 먼저 만들고 마지막에 자리를 잡는다 (쓰다 만 파일이 남지 않게)
    let tmp = pending.with_extension("part");
    let _ = std::fs::remove_file(&tmp);
    std::fs::copy(source, &tmp)?;

    // 베낀 것도 다시 확인한다
    let copied = check(&tmp);
    if !copied.ok {
        let _ = std::fs::remove_file(&tmp);
        return Err(AppError::new(
            "RESTORE_FAILED",
            "복원할 자료를 옮기는 중 문제가 생겼습니다. 지금 자료는 그대로입니다.",
        ));
    }
    let _ = std::fs::remove_file(&pending);
    std::fs::rename(&tmp, &pending)?;

    log::info!("restore staged (schema v{:?})", copied.schema_version);
    Ok(copied)
}

/// 놓아 둔 복원을 물린다.
pub fn cancel_restore(db_path: &Path) -> AppResult<bool> {
    let pending = pending_path(db_path);
    if pending.exists() {
        std::fs::remove_file(&pending)?;
        return Ok(true);
    }
    Ok(false)
}

pub fn restore_waiting(db_path: &Path) -> bool {
    pending_path(db_path).exists()
}

/// 앱을 열기 **전에** 부른다. 놓아 둔 복원이 있으면 지금 자료와 바꾼다.
///
/// 순서를 지킨다.
///   1. 놓아 둔 파일을 다시 확인한다 (그사이 망가졌을 수 있다)
///   2. 지금 자료를 `before_restore_*.db` 로 백업한다 — 되돌린 것이 생각과 다를 때 돌아올 곳
///   3. WAL 을 치우고 파일을 갈아 끼운다
///
/// 어느 단계에서 멈추든 지금 자료는 남는다. 돌려주는 값은 화면에 알릴 말이다.
pub fn apply_pending(db_path: &Path, now: DateTime<Local>) -> AppResult<Option<String>> {
    let pending = pending_path(db_path);
    if !pending.exists() {
        return Ok(None);
    }

    let result = check(&pending);
    if !result.ok {
        // 쓸 수 없는 파일이면 지금 자료를 건드리지 않고 옆에 치워 둔다
        let aside = db_path.with_file_name(format!("restore_failed_{}.db", stamp(now)));
        let _ = std::fs::rename(&pending, &aside);
        log::warn!("restore rejected before opening: {:?}", result.problem);
        return Err(AppError::new(
            "RESTORE_REJECTED",
            format!(
                "되돌리려던 파일을 쓸 수 없어 기존 자료를 그대로 열었습니다. {}",
                result.problem.unwrap_or_default()
            ),
        ));
    }

    // 지금 자료를 먼저 지킨다
    if db_path.exists() {
        let dir = db_path
            .parent()
            .ok_or_else(|| AppError::internal("자료 폴더를 찾을 수 없습니다."))?
            .join("backups");
        std::fs::create_dir_all(&dir)?;
        let safety = dir.join(file_name(Kind::BeforeRestore, now));

        // 여기서도 파일 복사가 아니라 SQLite 백업 — WAL 에 남은 내용까지 담는다
        let source = Connection::open(db_path)?;
        let mut target = Connection::open(&safety)?;
        {
            let b = rusqlite::backup::Backup::new(&source, &mut target)?;
            b.run_to_completion(500, std::time::Duration::ZERO, None)?;
        }
        let _: String =
            target.pragma_update_and_check(None, "journal_mode", "DELETE", |r| r.get(0))?;
        drop(target);
        drop(source);

        if !check(&safety).ok {
            let _ = std::fs::remove_file(&safety);
            return Err(AppError::new(
                "RESTORE_FAILED",
                "복원 전 백업을 만들지 못해 되돌리지 않았습니다. 기존 자료는 그대로입니다.",
            ));
        }
    }

    // 갈아 끼우기 — 남은 WAL 은 지금 자료의 것이라 함께 치운다
    for extra in ["-wal", "-shm"] {
        let mut p = db_path.as_os_str().to_os_string();
        p.push(extra);
        let _ = std::fs::remove_file(PathBuf::from(p));
    }
    std::fs::rename(&pending, db_path)?;

    log::info!("restore applied (schema v{:?})", result.schema_version);
    Ok(Some(format!(
        "백업에서 자료를 되돌렸습니다. 학생 {}명.",
        result.students.unwrap_or(0)
    )))
}

#[cfg(test)]
#[path = "backup_tests.rs"]
mod backup_tests;
