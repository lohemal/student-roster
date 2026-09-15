//! 마이그레이션 러너.
//!
//! `PRAGMA user_version`을 스키마 버전으로 쓰고, 아래 목록을 번호순으로 적용한다.
//! 적용 전에 기존 DB 파일을 `backups/`에 자동 복사한다.
//!
//! 새 마이그레이션 추가 방법
//!   1. `migrations/00N_설명.sql` 파일 생성
//!   2. 아래 MIGRATIONS 배열에 한 줄 추가
//! **v0.1.0 릴리스로 `001_init.sql` 은 동결했다.** 배포된 뒤에는 기존 파일을 절대
//! 수정하지 않는다 — 이미 그 구조로 자료를 만든 사용자가 있기 때문이다.

use std::path::Path;

use rusqlite::Connection;

use crate::error::{AppError, AppResult};

struct Migration {
    version: i32,
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "001_init",
    sql: include_str!("../../migrations/001_init.sql"),
}];

pub fn latest_version() -> i32 {
    MIGRATIONS.iter().map(|m| m.version).max().unwrap_or(0)
}

pub fn current_version(conn: &Connection) -> rusqlite::Result<i32> {
    conn.query_row("PRAGMA user_version", [], |r| r.get(0))
}

pub fn run(conn: &mut Connection, db_path: &Path) -> AppResult<()> {
    run_list(conn, db_path, MIGRATIONS)
}

/// 목록을 받아 적용한다. 검사에서 앞으로 더할 마이그레이션을 흉내 내는 데 쓴다.
fn run_list(conn: &mut Connection, db_path: &Path, list: &[Migration]) -> AppResult<()> {
    let from = current_version(conn)?;
    let to = list.iter().map(|m| m.version).max().unwrap_or(0);

    if from == to {
        return Ok(());
    }
    if from > to {
        return Err(AppError::new(
            "SCHEMA_TOO_NEW",
            "더 최신 버전의 프로그램에서 만든 자료입니다. 프로그램을 최신 버전으로 업데이트해 주세요.",
        )
        .detail(format!("db user_version={from}, app supports={to}")));
    }

    // 기존 자료가 있는 경우에만 백업 (최초 생성 시에는 백업할 것이 없음)
    if from > 0 {
        backup_before(conn, db_path, from)?;
    }

    for m in list.iter().filter(|m| m.version > from) {
        let tx = conn.transaction()?;
        tx.execute_batch(m.sql).map_err(|e| {
            AppError::new(
                "MIGRATION_FAILED",
                "자료 구조를 업데이트하지 못했습니다. 프로그램을 다시 시작해 주세요.",
            )
            .detail(format!("{} :: {e}", m.name))
        })?;
        tx.pragma_update(None, "user_version", m.version)?;
        tx.commit()?;
        log::info!("migration applied: {} (v{})", m.name, m.version);
    }

    Ok(())
}

/// 자료 구조를 바꾸기 직전의 자료를 통째로 떠 둔다.
///
/// **파일을 복사하지 않는다.** WAL 모드라 `.db` 만 베끼면 아직 반영되지 않은 내용이
/// 빠진다. 이미 열려 있는 연결로 SQLite 백업 API 를 쓴다.
fn backup_before(conn: &Connection, db_path: &Path, version: i32) -> AppResult<()> {
    if !db_path.exists() {
        return Ok(());
    }
    let dir = db_path
        .parent()
        .ok_or_else(|| AppError::internal("DB 경로에 상위 폴더가 없습니다."))?
        .join("backups");
    std::fs::create_dir_all(&dir)?;

    let stamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let path = dir.join(format!("before_migration_v{version}_{stamp}.db"));

    let mut out = Connection::open(&path)?;
    {
        let b = rusqlite::backup::Backup::new(conn, &mut out)?;
        b.run_to_completion(500, std::time::Duration::ZERO, None)?;
    }
    let _: String = out.pragma_update_and_check(None, "journal_mode", "DELETE", |r| r.get(0))?;
    log::info!("backup before migration v{version}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 빈_db에_최신_스키마가_만들어진다() {
        let mut conn = Connection::open_in_memory().unwrap();
        run(&mut conn, Path::new(":memory:")).unwrap();
        assert_eq!(current_version(&conn).unwrap(), latest_version());

        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        for t in [
            "settings",
            "school_years",
            "address_categories",
            "address_rules",
            "students",
            "enrollments",
            "enrollment_events",
            "graduations",
            "sibling_links",
            "issues",
            "imports",
            "year_transitions",
            "renumber_ops",
        ] {
            assert!(tables.iter().any(|x| x == t), "표 {t} 가 없다");
        }
    }

    #[test]
    fn 두_번_실행해도_아무_일도_없다() {
        let mut conn = Connection::open_in_memory().unwrap();
        run(&mut conn, Path::new(":memory:")).unwrap();
        run(&mut conn, Path::new(":memory:")).unwrap();
        assert_eq!(current_version(&conn).unwrap(), latest_version());
    }

    #[test]
    fn 더_새로운_자료는_열지_않는다() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", 999).unwrap();
        let err = run(&mut conn, Path::new(":memory:")).unwrap_err();
        assert_eq!(err.code, "SCHEMA_TOO_NEW");
    }

    /// 진짜 파일 DB 가 필요한 검사용 폴더.
    fn tmp(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "roster-migrate-{tag}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn 새_마이그레이션은_기존_자료를_지우지_않는다() {
        // v0.1.0 으로 만든 자료를 흉내 낸다
        let dir = tmp("next");
        let path = dir.join("studentroster.db");
        {
            let mut conn = Connection::open(&path).unwrap();
            run(&mut conn, &path).unwrap();
            conn.execute(
                "INSERT INTO students(name) VALUES ('가상학생')",
                [],
            )
            .unwrap();
        }

        // 다음 판에서 002 가 늘었다고 치고 그대로 적용해 본다
        let next = [
            Migration {
                version: 1,
                name: "001_init",
                sql: include_str!("../../migrations/001_init.sql"),
            },
            Migration {
                version: 2,
                name: "002_test_only",
                sql: "ALTER TABLE students ADD COLUMN memo TEXT;",
            },
        ];
        let mut conn = Connection::open(&path).unwrap();
        run_list(&mut conn, &path, &next).unwrap();

        assert_eq!(current_version(&conn).unwrap(), 2);
        let name: String = conn
            .query_row("SELECT name FROM students", [], |r| r.get(0))
            .unwrap();
        assert_eq!(name, "가상학생", "기존 자료가 그대로 있어야 한다");
        // 새 열도 생겼다
        conn.execute("UPDATE students SET memo = '메모'", []).unwrap();

        // 바꾸기 전 자료를 떠 두었는가
        let backups: Vec<String> = std::fs::read_dir(dir.join("backups"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert!(
            backups.iter().any(|n| n.starts_with("before_migration_v1_")),
            "{backups:?}"
        );
    }

    #[test]
    fn 이미_최신이면_다시_적용하지_않는다() {
        let dir = tmp("same");
        let path = dir.join("studentroster.db");
        let mut conn = Connection::open(&path).unwrap();
        run(&mut conn, &path).unwrap();
        run(&mut conn, &path).unwrap();

        // 두 번째 실행에서는 백업조차 만들지 않는다 (바꾼 것이 없으므로)
        assert!(!dir.join("backups").exists());
    }

    #[test]
    fn 기본_주소_분류가_들어_있다() {
        let mut conn = Connection::open_in_memory().unwrap();
        run(&mut conn, Path::new(":memory:")).unwrap();
        let names: Vec<String> = conn
            .prepare("SELECT name FROM address_categories ORDER BY sort_order")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(names, vec!["주택", "기타"]);
    }
}
