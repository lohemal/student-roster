//! Tauri 명령. 화면(`src/ipc/*.ts`)과 1:1 로 대응한다.
//!
//! 명령은 입력 검증과 `repo`/`domain` 호출만 하고, 규칙은 두지 않는다.

pub mod app;
pub mod issue;
pub mod settings;
pub mod student;
