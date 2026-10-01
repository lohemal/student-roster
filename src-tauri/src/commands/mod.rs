//! Tauri 명령. 화면(`src/ipc/*.ts`)과 1:1 로 대응한다.
//!
//! 명령은 입력 검증과 `repo`/`domain` 호출만 하고, 규칙은 두지 않는다.

use chrono::NaiveDate;

/// 화면이 묻는 **오늘** — 현재 재학생을 가리는 기준일.
///
/// 학교 업무는 날짜 단위라 시간은 보지 않는다. 기준일을 명령이 **들고 내려가므로**
/// repo 는 시스템 시계를 보지 않고, 검사는 기준일을 직접 넣어 아무 날짜나 세울 수 있다.
pub fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}

pub mod address;
pub mod app;
pub mod backup;
pub mod export;
pub mod graduation;
pub mod import;
pub mod issue;
pub mod renumber;
pub mod settings;
pub mod sibling;
pub mod stats;
pub mod student;
pub mod transfer;
pub mod transition;
