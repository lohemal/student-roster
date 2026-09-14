//! 저장소 계층. SQL은 여기에만 둔다.
//!
//! 함수는 `&rusqlite::Connection`을 받는다 — `Db::read`/`Db::write` 클로저 안에서
//! 호출되므로 트랜잭션 경계는 호출자가 정한다.

pub mod settings;
