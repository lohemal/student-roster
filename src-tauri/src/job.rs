//! 오래 걸리는 작업의 진행 상황 알림 (설계안 §7.2).
//!
//! 명령은 `jobId` 만 바로 돌려주고, 실제 일은 딴 갈래(thread)에서 한다.
//! 화면은 아래 세 가지를 듣는다.
//!
//! ```text
//! job://progress  { jobId, stage, stageLabel, done, total }
//! job://done      { jobId, result }
//! job://error     { jobId, error }
//! ```
//!
//! 가져오기·학년도 전환·주소 재적용이 모두 이 규약을 쓴다.

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::error::AppError;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress<'a> {
    pub job_id: &'a str,
    pub stage: &'a str,
    pub stage_label: &'a str,
    pub done: usize,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Done<T> {
    job_id: String,
    result: T,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Failed {
    job_id: String,
    error: AppError,
}

pub struct Job {
    app: AppHandle,
    id: String,
}

impl Job {
    pub fn new(app: AppHandle, id: impl Into<String>) -> Self {
        Self { app, id: id.into() }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    /// 어느 단계에서 몇 개를 마쳤는지 알린다.
    ///
    /// 알림이 실패해도(창이 닫혔다든지) 하던 일은 계속한다 — 화면 사정 때문에
    /// 자료 저장이 멈추면 안 된다.
    pub fn progress(&self, stage: &str, stage_label: &str, done: usize, total: usize) {
        let _ = self.app.emit(
            "job://progress",
            Progress {
                job_id: &self.id,
                stage,
                stage_label,
                done,
                total,
            },
        );
    }

    pub fn done<T: Serialize + Clone>(&self, result: T) {
        let _ = self.app.emit(
            "job://done",
            Done {
                job_id: self.id.clone(),
                result,
            },
        );
    }

    pub fn failed(&self, error: AppError) {
        let _ = self.app.emit(
            "job://error",
            Failed {
                job_id: self.id.clone(),
                error,
            },
        );
    }
}

/// 겹치지 않는 작업 번호.
pub fn new_id(prefix: &str) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{prefix}-{now}")
}
