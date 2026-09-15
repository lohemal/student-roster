//! 저장소 계층. SQL은 여기에만 둔다.
//!
//! 함수는 `&rusqlite::Connection`을 받는다 — `Db::read`/`Db::write` 클로저 안에서
//! 호출되므로 트랜잭션 경계는 호출자가 정한다.

pub mod address;
pub mod export;
pub mod graduation;
pub mod issue;
pub mod renumber;
pub mod settings;
pub mod sibling;
pub mod stats;
pub mod student;
pub mod transfer;
pub mod transition;
