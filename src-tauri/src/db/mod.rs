//! SQLite 연결 관리.
//!
//! 1인 사용 데스크톱 앱이므로 커넥션 풀 대신 `Mutex<Connection>` 하나로 충분하다.
//! 모든 DB 접근은 `Db::read` / `Db::write`를 통해서만 이루어진다.

pub mod backup;
pub mod migrate;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::Connection;

use crate::error::{AppError, AppResult};

pub struct Db {
    conn: Mutex<Connection>,
    path: PathBuf,
}

impl Db {
    /// DB 파일을 열고 필요한 마이그레이션을 적용한다.
    pub fn open(path: &Path) -> AppResult<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }

        let mut conn = Connection::open(path).map_err(|e| {
            AppError::new(
                "DB_OPEN_FAILED",
                "자료 파일을 열지 못했습니다. 프로그램을 다시 시작해 주세요.",
            )
            .detail(format!("{} :: {e}", path.display()))
        })?;

        setup_conn(&conn)?;
        migrate::run(&mut conn, path)?;

        Ok(Self {
            conn: Mutex::new(conn),
            path: path.to_path_buf(),
        })
    }

    /// 테스트용 메모리 DB (마이그레이션 적용됨).
    #[cfg(test)]
    pub fn memory() -> Self {
        let mut conn = Connection::open_in_memory().expect("메모리 DB를 열지 못했습니다");
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        migrate::run(&mut conn, Path::new(":memory:")).expect("마이그레이션 실패");
        Self {
            conn: Mutex::new(conn),
            path: PathBuf::from(":memory:"),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 읽기 전용 작업.
    pub fn read<T>(&self, f: impl FnOnce(&Connection) -> AppResult<T>) -> AppResult<T> {
        let guard = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        f(&guard)
    }

    /// 쓰기 작업. 클로저가 Err를 반환하면 전체가 롤백된다.
    pub fn write<T>(&self, f: impl FnOnce(&Connection) -> AppResult<T>) -> AppResult<T> {
        let mut guard = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let tx = guard.transaction()?;
        let out = f(&tx)?;
        tx.commit()?;
        Ok(out)
    }

    /// 자료가 성한가. `PRAGMA integrity_check` 가 'ok' 를 주면 성한 것이다.
    ///
    /// 앱을 열 때 한 번 본다 — 손상된 자료로 계속 쓰다가 백업까지 덮어쓰는 일을 막는다.
    pub fn integrity(&self) -> AppResult<String> {
        self.read(|c| {
            Ok(c.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))?)
        })
    }

    /// 현재 스키마 버전.
    pub fn schema_version(&self) -> AppResult<i32> {
        self.read(|c| Ok(migrate::current_version(c)?))
    }

    /// 백업 등이 놓이는 폴더 (`<자료 폴더>/backups`).
    pub fn backup_dir(&self) -> PathBuf {
        self.path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("backups")
    }

    /// 내보내기 파일이 놓이는 폴더.
    pub fn export_dir(&self) -> PathBuf {
        self.path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("exports")
    }

    /// 지금 자료를 통째로 `dest`에 복사한다 (SQLite 백업 API — WAL 내용 포함).
    pub fn backup_to(&self, dest: &Path) -> AppResult<()> {
        if let Some(dir) = dest.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let guard = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut out = Connection::open(dest)?;
        {
            let b = rusqlite::backup::Backup::new(&guard, &mut out)?;
            b.run_to_completion(500, std::time::Duration::ZERO, None)?;
        }
        // 백업본은 한 파일로 — WAL을 끈다.
        let _: String =
            out.pragma_update_and_check(None, "journal_mode", "DELETE", |r| r.get(0))?;
        Ok(())
    }
}

fn setup_conn(conn: &Connection) -> AppResult<()> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(())
}
