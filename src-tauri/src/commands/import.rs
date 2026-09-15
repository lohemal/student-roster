//! 가져오기 명령.
//!
//! 분석까지는 바로 답하고, 저장은 딴 갈래에서 돌리며 진행 상황을 알린다(§7.2).

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::error::{AppError, AppResult};
use crate::import::{
    analyze::{self, AnalyzeResult},
    apply,
    excel::{self, FileInfo, PREVIEW_ROWS},
    field_list,
    mapping::{self, Field, Mapping},
    row, FieldInfo, Session,
};
use crate::job::{self, Job};
use crate::AppState;

/// 한 번에 다룰 수 있는 최대 줄 수. 학교 규모를 훨씬 넘으면 잘못 고른 파일일 가능성이 높다.
const MAX_ROWS: usize = 20_000;

// ---------------------------------------------------------------
// 1단계 — 파일 살펴보기
// ---------------------------------------------------------------

#[tauri::command]
pub fn import_inspect(path: String) -> AppResult<FileInfo> {
    excel::inspect(&path)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetPreview {
    /// 헤더로 본 줄 (0부터, 빈 줄을 뺀 뒤 기준)
    pub header_row: usize,
    pub headers: Vec<String>,
    pub sample_rows: Vec<Vec<String>>,
    pub total_rows: usize,
    /// 별칭표로 추측한 짝짓기
    pub mapping: Mapping,
    pub fields: Vec<FieldInfo>,
}

/// 시트를 열어 헤더를 찾고 짝짓기를 추측한다.
#[tauri::command]
pub fn import_preview(
    path: String,
    sheet: String,
    header_row: Option<usize>,
) -> AppResult<SheetPreview> {
    let rows = excel::read_sheet(&path, &sheet)?;
    if rows.is_empty() {
        return Err(AppError::invalid(
            "선택한 시트가 비어 있습니다. 다른 시트를 골라 주세요.",
        ));
    }

    let header_row = header_row.unwrap_or_else(|| mapping::guess_header_row(&rows, 10));
    let header_row = header_row.min(rows.len() - 1);
    let headers = rows[header_row].1.clone();

    let body = &rows[header_row + 1..];
    Ok(SheetPreview {
        header_row,
        mapping: mapping::guess(&headers),
        headers,
        sample_rows: body
            .iter()
            .take(PREVIEW_ROWS)
            .map(|(_, cells)| cells.clone())
            .collect(),
        total_rows: body.len(),
        fields: field_list(),
    })
}

// ---------------------------------------------------------------
// 2단계 — 분석 (DB를 바꾸지 않는다)
// ---------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeInput {
    pub path: String,
    pub sheet: String,
    pub header_row: usize,
    pub mapping: Mapping,
    pub school_year: i32,
    /// 학년 열이 없을 때 쓸 학년
    pub default_grade: Option<i32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeResponse {
    /// 이 분석 결과의 번호. 적용할 때 그대로 돌려준다
    pub analysis_id: String,
    pub file_name: String,
    pub sheet_name: String,
    pub school_year: i32,
    #[serde(flatten)]
    pub result: AnalyzeResult,
}

#[tauri::command]
pub fn import_analyze(
    state: State<'_, AppState>,
    input: AnalyzeInput,
) -> AppResult<AnalyzeResponse> {
    // 막는 조건을 먼저 본다 — 사람이 고칠 수 있는 말로 알려 준다
    if !input.mapping.contains_key(&Field::Name) {
        return Err(AppError::invalid(
            "이름 열을 지정해 주세요. 이름이 없으면 학생을 가져올 수 없습니다.",
        ));
    }
    check_year(state.inner(), input.school_year)?;

    let all = excel::read_sheet(&input.path, &input.sheet)?;
    if input.header_row >= all.len() {
        return Err(AppError::invalid(
            "헤더 줄을 찾지 못했습니다. 시트를 다시 골라 주세요.",
        ));
    }
    let body = &all[input.header_row + 1..];
    if body.is_empty() {
        return Err(AppError::invalid(
            "헤더 아래에 자료가 없습니다. 헤더 줄이 맞는지 확인해 주세요.",
        ));
    }
    if body.len() > MAX_ROWS {
        return Err(AppError::invalid(format!(
            "줄이 {}개입니다. 한 번에 {MAX_ROWS}줄까지 가져올 수 있습니다.",
            body.len()
        )));
    }

    let today = chrono::Local::now().date_naive();
    let rows: Vec<_> = body
        .iter()
        .map(|(n, cells)| {
            row::parse(*n, cells, &input.mapping, input.default_grade, today)
        })
        .collect();

    let result = state
        .db
        .read(|c| analyze::run(c, &rows, input.school_year, |_, _| {}))?;

    let file_name = std::path::Path::new(&input.path)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| input.path.clone());

    let analysis_id = job::new_id("import");
    state.import.put(Session {
        id: analysis_id.clone(),
        file_name: file_name.clone(),
        sheet_name: input.sheet.clone(),
        school_year: input.school_year,
        mapping: input.mapping,
        rows,
        running: false,
    });

    Ok(AnalyzeResponse {
        analysis_id,
        file_name,
        sheet_name: input.sheet,
        school_year: input.school_year,
        result,
    })
}

// ---------------------------------------------------------------
// 3단계 — 적용 (딴 갈래에서 돌린다)
// ---------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyInput {
    pub analysis_id: String,
    #[serde(default)]
    pub choice: apply::Choice,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyStarted {
    pub job_id: String,
    pub total: usize,
}

/// 저장을 시작한다. 바로 돌아오고, 끝은 `job://done` 으로 알린다.
#[tauri::command]
pub fn import_apply(
    app: AppHandle,
    state: State<'_, AppState>,
    input: ApplyInput,
) -> AppResult<ApplyStarted> {
    let session = state.import.start(&input.analysis_id)?;
    let total = session.rows.len();

    let db = state.db.clone();
    let store = state.import.clone();
    let job = Job::new(app, session.id.clone());
    let job_id = job.id().to_string();
    let choice = input.choice;

    std::thread::spawn(move || {
        let today = chrono::Local::now().date_naive();
        let meta = apply::Meta {
            file_name: session.file_name.clone(),
            sheet_name: session.sheet_name.clone(),
            school_year: session.school_year,
            mapping_json: serde_json::to_string(&session.mapping).unwrap_or_default(),
        };

        // 한 트랜잭션 — 중간에 실패하면 하나도 들어가지 않는다
        let outcome = db.write(|c| {
            apply::run(c, &session.rows, &meta, &choice, today, |stage, label, done, total| {
                job.progress(stage, label, done, total)
            })
        });

        match outcome {
            Ok(result) => {
                store.finish(false); // 같은 분석을 두 번 넣지 못하게 자리를 비운다
                job.done(result);
            }
            Err(e) => {
                store.finish(true); // 고쳐서 다시 시도할 수 있게 남겨 둔다
                job.failed(e);
            }
        }
    });

    Ok(ApplyStarted { job_id, total })
}

/// 가져오기 기록.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportRow {
    pub id: i64,
    pub file_name: String,
    pub sheet_name: Option<String>,
    pub school_year: i32,
    pub mode: String,
    pub imported_at: String,
    pub total: i64,
    pub added: i64,
    pub updated: i64,
    pub skipped: i64,
    pub flagged: i64,
}

#[tauri::command]
pub fn import_history(state: State<'_, AppState>, limit: Option<i64>) -> AppResult<Vec<ImportRow>> {
    let limit = limit.unwrap_or(10).clamp(1, 100);
    state.db.read(|c| {
        let mut st = c.prepare(
            "SELECT id, file_name, sheet_name, school_year, mode, imported_at,
                    total, added, updated, skipped, flagged
               FROM imports ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = st
            .query_map([limit], |r| {
                Ok(ImportRow {
                    id: r.get(0)?,
                    file_name: r.get(1)?,
                    sheet_name: r.get(2)?,
                    school_year: r.get(3)?,
                    mode: r.get(4)?,
                    imported_at: r.get(5)?,
                    total: r.get(6)?,
                    added: r.get(7)?,
                    updated: r.get(8)?,
                    skipped: r.get(9)?,
                    flagged: r.get(10)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(rows)
    })
}

fn check_year(state: &AppState, year: i32) -> AppResult<()> {
    let ok = state.db.read(|c| {
        let n: i64 = c.query_row(
            "SELECT COUNT(*) FROM school_years WHERE year = ?1",
            [year],
            |r| r.get(0),
        )?;
        Ok(n > 0)
    })?;
    if !ok {
        return Err(AppError::setup_required(format!(
            "{year}학년도가 없습니다. 설정에서 학년도를 먼저 만들어 주세요."
        )));
    }
    Ok(())
}
