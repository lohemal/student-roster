//! 백업·복원 검사.
//!
//! 여기서 보는 것 둘: **되돌릴 수 있는가**, 그리고 **되돌리다 실패해도 지금 자료가
//! 남는가.** 학생 자료는 이 컴퓨터에만 있으므로 이 두 가지가 마지막 안전장치다.
//! 이름은 모두 가상이다.

use super::*;
use crate::repo::{settings, student};
use chrono::{Duration, Local, NaiveDate, TimeZone};

fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 9, 15).unwrap()
}

fn at(day: u32, hour: u32) -> chrono::DateTime<Local> {
    Local
        .with_ymd_and_hms(2026, 9, day, hour, 0, 0)
        .single()
        .expect("있는 시각")
}

/// 빈 폴더 하나.
fn tmp(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "roster-backup-{tag}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 학생 `n` 명이 든 진짜 파일 DB.
fn db_with(dir: &Path, n: usize) -> Db {
    let db = Db::open(&dir.join("studentroster.db")).unwrap();
    db.write(|c| {
        settings::create_year(c, 2026)?;
        settings::set_current_year(c, 2026)
    })
    .unwrap();
    add_students(&db, 0, n);
    db
}

fn add_students(db: &Db, from: usize, count: usize) {
    db.write(|c| {
        for i in from..from + count {
            student::create(
                c,
                &student::StudentInput {
                    name: format!("학생{i:03}"),
                    gender: Some("M".into()),
                    birth_raw: Some("170315".into()),
                    school_year: 2026,
                    grade: (i % 6) as i32 + 1,
                    class_name: Some("가람".into()),
                    class_no: Some((i % 30) as i32 + 1),
                    ..Default::default()
                },
                today(),
            )?;
        }
        Ok(())
    })
    .unwrap();
}

fn students_in(path: &Path) -> i64 {
    let c = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    c.query_row("SELECT COUNT(*) FROM students", [], |r| r.get(0))
        .unwrap()
}

// ---------------------------------------------------------------
// 이름과 종류
// ---------------------------------------------------------------

#[test]
fn 파일_이름으로_종류를_가린다() {
    assert_eq!(Kind::of_file("auto_20260915_080000.db"), Kind::Auto);
    assert_eq!(Kind::of_file("manual_20260915_185000.db"), Kind::Manual);
    assert_eq!(
        Kind::of_file("before_restore_20260915_190000.db"),
        Kind::BeforeRestore
    );
    assert_eq!(
        Kind::of_file("before_year_transition_2026_to_2027_20260915_183533.db"),
        Kind::BeforeTransition
    );
    assert_eq!(
        Kind::of_file("before_migration_v1_20260915_183533.db"),
        Kind::BeforeMigration
    );
    assert_eq!(Kind::of_file("내가_따로_둔_파일.db"), Kind::Other);
}

#[test]
fn 자동_백업만_자동_정리_대상이다() {
    assert!(Kind::Auto.cleaned_up());
    for k in [
        Kind::Manual,
        Kind::BeforeRestore,
        Kind::BeforeMigration,
        Kind::BeforeTransition,
        Kind::Other,
    ] {
        assert!(!k.cleaned_up(), "{}", k.label());
    }
}

// ---------------------------------------------------------------
// 만들기
// ---------------------------------------------------------------

#[test]
fn 수동_백업은_만들고_바로_확인한다() {
    let dir = tmp("manual");
    let db = db_with(&dir, 100);

    let entry = create(&db, Kind::Manual, at(15, 18)).unwrap();
    assert_eq!(entry.kind, Kind::Manual);
    assert!(entry.file_name.starts_with("manual_"));
    assert!(entry.check.ok, "{:?}", entry.check.problem);
    assert_eq!(entry.check.students, Some(100));
    assert_eq!(entry.check.schema_version, Some(migrate::latest_version()));
    assert!(entry.size > 0);
    assert_eq!(entry.created_at, "2026.09.15. 18:00:00");
}

#[test]
fn 백업은_아직_반영되지_않은_내용까지_담는다() {
    // WAL 모드라 .db 파일만 베끼면 방금 저장한 것이 빠질 수 있다
    let dir = tmp("wal");
    let db = db_with(&dir, 10);
    add_students(&db, 10, 5); // 체크포인트 없이 바로 백업

    let entry = create(&db, Kind::Manual, at(15, 18)).unwrap();
    assert_eq!(entry.check.students, Some(15), "WAL 내용이 빠졌다");
    assert_eq!(students_in(Path::new(&entry.path)), 15);
}

#[test]
fn 백업_파일은_한_덩어리다() {
    let dir = tmp("single");
    let db = db_with(&dir, 3);
    let entry = create(&db, Kind::Manual, at(15, 18)).unwrap();

    // 백업 옆에 -wal 이 남아 있으면 USB 로 하나만 옮겼을 때 자료가 어긋난다
    for extra in ["-wal", "-shm"] {
        let p = PathBuf::from(format!("{}{extra}", entry.path));
        assert!(!p.exists(), "{p:?} 가 남아 있다");
    }
}

// ---------------------------------------------------------------
// 자동 백업과 보존
// ---------------------------------------------------------------

#[test]
fn 같은_날_여러_번_열어도_자동_백업은_하나다() {
    let dir = tmp("auto");
    let db = db_with(&dir, 5);

    let first = auto_if_needed(&db, today(), at(15, 8)).unwrap();
    assert!(first.is_some());
    let second = auto_if_needed(&db, today(), at(15, 14)).unwrap();
    assert!(second.is_none(), "같은 날 두 번 만들지 않는다");

    let autos = list(&db.backup_dir())
        .unwrap()
        .into_iter()
        .filter(|e| e.kind == Kind::Auto)
        .count();
    assert_eq!(autos, 1);
}

#[test]
fn 날짜가_바뀌면_자동_백업을_다시_만든다() {
    let dir = tmp("auto2");
    let db = db_with(&dir, 5);
    auto_if_needed(&db, today(), at(15, 8)).unwrap();

    let next = today() + Duration::days(1);
    assert!(auto_if_needed(&db, next, at(16, 8)).unwrap().is_some());
}

#[test]
fn 자동_백업만_오래된_것부터_정리한다() {
    let dir = tmp("keep");
    let db = db_with(&dir, 5);
    let backups = db.backup_dir();

    // 자동 31개 + 수동 1개 + 학년도 전환 전 1개
    for i in 0..31 {
        let name = format!("auto_202609{:02}_0800{:02}.db", (i % 28) + 1, i);
        create(&db, Kind::Manual, at(15, 18)).unwrap();
        // 이름만 자동 백업으로 바꿔 둔다 (시각이 서로 다르게)
        let made = list(&backups)
            .unwrap()
            .into_iter()
            .find(|e| e.kind == Kind::Manual)
            .unwrap();
        std::fs::rename(&made.path, backups.join(name)).unwrap();
    }
    create(&db, Kind::Manual, at(15, 19)).unwrap();
    std::fs::copy(
        backups.join(file_name(Kind::Manual, at(15, 19))),
        backups.join("before_year_transition_2026_to_2027_20260915_183533.db"),
    )
    .unwrap();

    let before = list(&backups).unwrap();
    assert_eq!(before.iter().filter(|e| e.kind == Kind::Auto).count(), 31);

    let gone = cleanup_auto(&backups, KEEP_AUTO).unwrap();
    assert_eq!(gone, 1, "30개만 남긴다");

    let after = list(&backups).unwrap();
    assert_eq!(after.iter().filter(|e| e.kind == Kind::Auto).count(), 30);
    assert_eq!(
        after.iter().filter(|e| e.kind == Kind::Manual).count(),
        1,
        "수동 백업은 건드리지 않는다"
    );
    assert_eq!(
        after.iter().filter(|e| e.kind == Kind::BeforeTransition).count(),
        1,
        "학년도 전환 전 백업은 건드리지 않는다"
    );
}

#[test]
fn 자동_백업이_서른개_이하면_아무것도_지우지_않는다() {
    let dir = tmp("keep2");
    let db = db_with(&dir, 2);
    auto_if_needed(&db, today(), at(15, 8)).unwrap();
    assert_eq!(cleanup_auto(&db.backup_dir(), KEEP_AUTO).unwrap(), 0);
}

// ---------------------------------------------------------------
// 확인
// ---------------------------------------------------------------

#[test]
fn 엑셀이나_엉뚱한_파일은_백업으로_보지_않는다() {
    let dir = tmp("bad");
    let path = dir.join("이상한파일.db");
    std::fs::write(&path, "이건 SQLite 파일이 아니다".as_bytes()).unwrap();

    let result = check(&path);
    assert!(!result.ok);
    assert!(result.problem.is_some());
}

#[test]
fn 다른_프로그램의_sqlite_는_거절한다() {
    let dir = tmp("other");
    let path = dir.join("다른앱.db");
    {
        let c = Connection::open(&path).unwrap();
        c.execute("CREATE TABLE memo(id INTEGER PRIMARY KEY, body TEXT)", [])
            .unwrap();
    }

    let result = check(&path);
    assert!(!result.ok);
    assert!(
        result.problem.unwrap().contains("이 프로그램의 자료"),
        "무엇이 문제인지 말해 줘야 한다"
    );
}

#[test]
fn 더_최신_구조의_자료는_되돌리지_않는다() {
    let dir = tmp("newer");
    let db = db_with(&dir, 3);
    let entry = create(&db, Kind::Manual, at(15, 18)).unwrap();
    {
        let c = Connection::open(&entry.path).unwrap();
        c.pragma_update(None, "user_version", 999).unwrap();
    }

    let result = check(Path::new(&entry.path));
    assert!(!result.ok);
    assert_eq!(result.schema_version, Some(999));
    assert!(result.problem.unwrap().contains("업데이트"));
}

// ---------------------------------------------------------------
// 복원
// ---------------------------------------------------------------

#[test]
fn 백업으로_되돌리면_그때_인원으로_돌아온다() {
    let dir = tmp("restore");
    let db_path = dir.join("studentroster.db");
    let db = db_with(&dir, 100);
    let saved = create(&db, Kind::Manual, at(15, 18)).unwrap();

    add_students(&db, 100, 20);
    assert_eq!(db.read(|c| Ok(count(c))).unwrap(), 120);

    // 놓아 두기 — 아직 지금 자료는 그대로다
    stage_restore(&db_path, Path::new(&saved.path)).unwrap();
    assert_eq!(db.read(|c| Ok(count(c))).unwrap(), 120, "아직 바뀌지 않는다");

    // 연결을 닫고 다음 실행을 흉내 낸다
    drop(db);
    let note = apply_pending(&db_path, at(15, 19)).unwrap();
    assert!(note.unwrap().contains("100"));

    let back = Db::open(&db_path).unwrap();
    assert_eq!(back.read(|c| Ok(count(c))).unwrap(), 100);
    assert_eq!(back.integrity().unwrap(), "ok");
}

fn count(c: &Connection) -> i64 {
    c.query_row("SELECT COUNT(*) FROM students", [], |r| r.get(0))
        .unwrap()
}

#[test]
fn 되돌리기_직전에_지금_자료를_한_번_더_백업한다() {
    let dir = tmp("safety");
    let db_path = dir.join("studentroster.db");
    let db = db_with(&dir, 10);
    let saved = create(&db, Kind::Manual, at(15, 18)).unwrap();
    add_students(&db, 10, 7); // 17명

    stage_restore(&db_path, Path::new(&saved.path)).unwrap();
    drop(db);
    apply_pending(&db_path, at(15, 19)).unwrap();

    let safety = list(&dir.join("backups"))
        .unwrap()
        .into_iter()
        .find(|e| e.kind == Kind::BeforeRestore)
        .expect("복원 전 백업이 있어야 한다");
    assert!(safety.check.ok);
    assert_eq!(
        safety.check.students,
        Some(17),
        "되돌리기 직전 상태여야 한다"
    );
    assert!(!safety.kind.cleaned_up(), "자동 정리 대상이 아니다");
}

#[test]
fn 되돌릴_수_없는_파일이면_지금_자료를_건드리지_않는다() {
    let dir = tmp("keepnow");
    let db_path = dir.join("studentroster.db");
    let db = db_with(&dir, 42);

    let junk = dir.join("엉뚱한파일.db");
    std::fs::write(&junk, b"not sqlite").unwrap();

    let err = stage_restore(&db_path, &junk).unwrap_err();
    assert_eq!(err.code, "INVALID_INPUT");
    assert!(!restore_waiting(&db_path), "놓아 두지도 않는다");
    assert_eq!(db.read(|c| Ok(count(c))).unwrap(), 42);
}

#[test]
fn 준비해_둔_복원을_물릴_수_있다() {
    let dir = tmp("cancel");
    let db_path = dir.join("studentroster.db");
    let db = db_with(&dir, 8);
    let saved = create(&db, Kind::Manual, at(15, 18)).unwrap();

    stage_restore(&db_path, Path::new(&saved.path)).unwrap();
    assert!(restore_waiting(&db_path));
    assert!(cancel_restore(&db_path).unwrap());
    assert!(!restore_waiting(&db_path));

    drop(db);
    assert!(apply_pending(&db_path, at(15, 19)).unwrap().is_none());
    let back = Db::open(&db_path).unwrap();
    assert_eq!(back.read(|c| Ok(count(c))).unwrap(), 8);
}

#[test]
fn 놓아_둔_파일이_망가지면_기존_자료를_그대로_연다() {
    let dir = tmp("brokenpending");
    let db_path = dir.join("studentroster.db");
    let db = db_with(&dir, 55);
    let saved = create(&db, Kind::Manual, at(15, 18)).unwrap();
    stage_restore(&db_path, Path::new(&saved.path)).unwrap();
    drop(db);

    // 다음 실행 전에 파일이 깨졌다
    let pending = PathBuf::from(format!("{}{}", db_path.display(), ".restore-pending"));
    std::fs::write(&pending, b"broken").unwrap();

    let err = apply_pending(&db_path, at(15, 19)).unwrap_err();
    assert_eq!(err.code, "RESTORE_REJECTED");

    let back = Db::open(&db_path).unwrap();
    assert_eq!(back.read(|c| Ok(count(c))).unwrap(), 55, "기존 자료가 그대로다");
    assert!(!restore_waiting(&db_path), "쓸 수 없는 파일은 치워 둔다");
}

#[test]
fn 되돌릴_것이_없으면_아무것도_하지_않는다() {
    let dir = tmp("nopending");
    let db_path = dir.join("studentroster.db");
    let db = db_with(&dir, 3);
    drop(db);
    assert!(apply_pending(&db_path, at(15, 19)).unwrap().is_none());
}

#[test]
fn 다른_곳에_보관한_백업도_되돌릴_수_있다() {
    let dir = tmp("outside");
    let db_path = dir.join("studentroster.db");
    let db = db_with(&dir, 31);
    let saved = create(&db, Kind::Manual, at(15, 18)).unwrap();

    // USB 에 옮겨 둔 것처럼 백업 폴더 밖으로 복사
    let usb = tmp("usb");
    let far = usb.join("우리학교_백업.db");
    std::fs::copy(&saved.path, &far).unwrap();

    add_students(&db, 31, 4);
    stage_restore(&db_path, &far).unwrap();
    drop(db);
    apply_pending(&db_path, at(15, 19)).unwrap();

    let back = Db::open(&db_path).unwrap();
    assert_eq!(back.read(|c| Ok(count(c))).unwrap(), 31);
}

// ---------------------------------------------------------------
// 목록
// ---------------------------------------------------------------

#[test]
fn 목록은_새_것부터_보여_준다() {
    let dir = tmp("list");
    let db = db_with(&dir, 4);
    create(&db, Kind::Manual, at(14, 9)).unwrap();
    create(&db, Kind::Manual, at(15, 9)).unwrap();
    create(&db, Kind::Auto, at(15, 8)).unwrap();

    let rows = list(&db.backup_dir()).unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].created_at, "2026.09.15. 09:00:00");
    assert_eq!(rows[2].created_at, "2026.09.14. 09:00:00");
    assert!(rows.iter().all(|e| e.check.ok));
}

#[test]
fn 성하지_않은_백업은_목록에서_표시된다() {
    let dir = tmp("listbad");
    let db = db_with(&dir, 4);
    create(&db, Kind::Manual, at(15, 9)).unwrap();
    std::fs::write(db.backup_dir().join("manual_20260914_090000.db"), b"broken").unwrap();

    let rows = list(&db.backup_dir()).unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows.iter().filter(|e| e.check.ok).count(), 1);
    let bad = rows.iter().find(|e| !e.check.ok).unwrap();
    assert!(bad.check.problem.is_some());
}

// ---------------------------------------------------------------
// 빠르기
// ---------------------------------------------------------------

#[test]
fn 천명짜리_자료도_금방_백업하고_되돌린다() {
    use std::time::Instant;

    let dir = tmp("speed");
    let db_path = dir.join("studentroster.db");
    let db = db_with(&dir, 1000);

    let t0 = Instant::now();
    let entry = create(&db, Kind::Manual, at(15, 18)).unwrap();
    let made = t0.elapsed();
    assert!(entry.check.ok);

    add_students(&db, 1000, 50);
    let t1 = Instant::now();
    stage_restore(&db_path, Path::new(&entry.path)).unwrap();
    let staged = t1.elapsed();

    drop(db);
    let t2 = Instant::now();
    apply_pending(&db_path, at(15, 19)).unwrap();
    let applied = t2.elapsed();

    let size = std::fs::metadata(&entry.path).map(|m| m.len()).unwrap_or(0);
    println!(
        "1,000명({size} bytes) — 백업+확인 {made:?} · 복원 준비 {staged:?} · 복원 적용(안전 백업 포함) {applied:?}"
    );
    assert!(made.as_millis() < 5000, "백업 {made:?}");
    assert!(applied.as_millis() < 5000, "복원 {applied:?}");

    let back = Db::open(&db_path).unwrap();
    assert_eq!(back.read(|c| Ok(count(c))).unwrap(), 1000);
}
