//! settings(key-value) · school_years.

use rusqlite::{params, Connection, OptionalExtension};

use crate::error::{AppError, AppResult};

pub fn get(c: &Connection, key: &str) -> AppResult<Option<String>> {
    Ok(c.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        [key],
        |r| r.get(0),
    )
    .optional()?)
}

pub fn set(c: &Connection, key: &str, value: &str) -> AppResult<()> {
    c.execute(
        "INSERT INTO settings(key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value,
                                       updated_at = datetime('now','localtime')",
        params![key, value],
    )?;
    Ok(())
}

pub struct SchoolYear {
    pub year: i32,
    pub is_current: bool,
    pub student_count: i64,
    pub created_at: String,
}

pub fn list_years(c: &Connection) -> AppResult<Vec<SchoolYear>> {
    let mut st = c.prepare(
        "SELECT y.year, y.is_current, y.created_at,
                (SELECT COUNT(*) FROM enrollments e
                  WHERE e.school_year = y.year AND e.status IN ('ENROLLED','TRANSFER_IN'))
           FROM school_years y
          ORDER BY y.year DESC",
    )?;
    let rows = st
        .query_map([], |r| {
            Ok(SchoolYear {
                year: r.get(0)?,
                is_current: r.get::<_, i32>(1)? == 1,
                created_at: r.get(2)?,
                student_count: r.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

pub fn current_year(c: &Connection) -> AppResult<Option<i32>> {
    Ok(c.query_row(
        "SELECT year FROM school_years WHERE is_current = 1",
        [],
        |r| r.get(0),
    )
    .optional()?)
}

/// 현재 학년도. 없으면 초기 설정이 필요하다는 오류.
#[allow(dead_code)] // Phase 1 학생 명령에서 쓴다
pub fn require_current_year(c: &Connection) -> AppResult<i32> {
    current_year(c)?.ok_or_else(|| {
        AppError::setup_required("먼저 설정에서 현재 학년도를 지정해 주세요.")
    })
}

pub fn create_year(c: &Connection, year: i32) -> AppResult<()> {
    c.execute(
        "INSERT INTO school_years(year) VALUES (?1) ON CONFLICT(year) DO NOTHING",
        [year],
    )?;
    Ok(())
}

pub fn set_current_year(c: &Connection, year: i32) -> AppResult<()> {
    let exists: i64 = c.query_row(
        "SELECT COUNT(*) FROM school_years WHERE year = ?1",
        [year],
        |r| r.get(0),
    )?;
    if exists == 0 {
        return Err(AppError::not_found(format!(
            "{year}학년도가 없습니다. 먼저 학년도를 만들어 주세요."
        )));
    }
    // 부분 유니크 인덱스 때문에 먼저 모두 내리고 하나만 올린다
    c.execute("UPDATE school_years SET is_current = 0 WHERE is_current = 1", [])?;
    c.execute("UPDATE school_years SET is_current = 1 WHERE year = ?1", [year])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    #[test]
    fn 설정값을_저장하고_읽는다() {
        let db = Db::memory();
        db.write(|c| set(c, "school_name", "한솔초등학교")).unwrap();
        let v = db.read(|c| get(c, "school_name")).unwrap();
        assert_eq!(v.as_deref(), Some("한솔초등학교"));

        db.write(|c| set(c, "school_name", "새솔초등학교")).unwrap();
        let v = db.read(|c| get(c, "school_name")).unwrap();
        assert_eq!(v.as_deref(), Some("새솔초등학교"), "덮어쓰기");
    }

    #[test]
    fn 현재_학년도는_하나만_된다() {
        let db = Db::memory();
        assert_eq!(db.read(current_year).unwrap(), None);

        db.write(|c| {
            create_year(c, 2026)?;
            set_current_year(c, 2026)
        })
        .unwrap();
        assert_eq!(db.read(current_year).unwrap(), Some(2026));

        db.write(|c| {
            create_year(c, 2027)?;
            set_current_year(c, 2027)
        })
        .unwrap();
        assert_eq!(db.read(current_year).unwrap(), Some(2027));

        let years = db.read(list_years).unwrap();
        assert_eq!(years.len(), 2);
        assert!(years.iter().filter(|y| y.is_current).count() == 1);
    }

    #[test]
    fn 없는_학년도를_현재로_못_만든다() {
        let db = Db::memory();
        let err = db.write(|c| set_current_year(c, 2030)).unwrap_err();
        assert_eq!(err.code, "NOT_FOUND");
    }

    #[test]
    fn 현재_학년도가_없으면_설정_필요_오류() {
        let db = Db::memory();
        let err = db.read(require_current_year).unwrap_err();
        assert_eq!(err.code, "SETUP_REQUIRED");
    }
}
