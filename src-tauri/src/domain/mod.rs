//! 순수 계산 규칙. DB·Tauri에 의존하지 않는다.
//!
//! 생년월일·연락처·주소 정규화, 형제 판정, 번호 재정렬, 진급 계산이 여기에 들어온다.
//! 모든 함수는 단위 테스트를 가진다.

pub mod address;
pub mod birth;
pub mod korean;
pub mod label;
pub mod phone;
pub mod renumber;
pub mod sibling;
