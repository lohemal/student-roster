import { listen } from '@tauri-apps/api/event'

import { invoke } from './invoke'

/** Rust `mapping::Field` 와 같은 값. 항목 목록의 원본은 Rust에 있다. */
export type FieldKey =
  | 'grade'
  | 'className'
  | 'classNo'
  | 'name'
  | 'gender'
  | 'birth'
  | 'address'
  | 'fatherName'
  | 'motherName'
  | 'fatherPhone'
  | 'motherPhone'
  | 'primaryPhone'
  | 'note'

export interface FieldInfo {
  key: FieldKey
  label: string
  required: boolean
}

/** 항목 → 엑셀 열 번호. 없으면 '사용 안 함' */
export type Mapping = Partial<Record<FieldKey, number>>

export interface SheetInfo {
  name: string
  rows: number
  cols: number
}

export interface FileInfo {
  fileName: string
  sheets: SheetInfo[]
}

export interface SheetPreview {
  headerRow: number
  headers: string[]
  sampleRows: string[][]
  totalRows: number
  mapping: Mapping
  fields: FieldInfo[]
}

export type Action = 'add' | 'update' | 'unchanged' | 'ambiguous' | 'blocked'

export interface FieldChange {
  field: string
  before: string
  after: string
}

export interface Candidate {
  studentId: number
  name: string
  birth: string
  whereAt: string
}

export interface RowWarning {
  message: string
}

export interface RowPlan {
  excelRow: number
  action: Action
  actionLabel: string
  name: string
  whereAt: string
  birth: string
  studentId: number | null
  changes: FieldChange[]
  reason: string | null
  candidates: Candidate[]
  warnings: RowWarning[]
}

export interface AnalyzeSummary {
  total: number
  add: number
  update: number
  unchanged: number
  ambiguous: number
  blocked: number
  warned: number
}

export interface AnalyzeResponse {
  analysisId: string
  fileName: string
  sheetName: string
  schoolYear: number
  summary: AnalyzeSummary
  rows: RowPlan[]
}

export interface ApplyChoice {
  add: boolean
  update: boolean
  skipRows: number[]
}

export interface ApplyStarted {
  jobId: string
  total: number
}

export interface ApplyResult {
  total: number
  added: number
  updated: number
  unchanged: number
  skipped: number
  issueCount: number
  importId: number
}

export interface ImportHistoryRow {
  id: number
  fileName: string
  sheetName: string | null
  schoolYear: number
  mode: string
  importedAt: string
  total: number
  added: number
  updated: number
  skipped: number
  flagged: number
}

export const importApi = {
  inspect: (path: string) => invoke<FileInfo>('import_inspect', { path }),
  preview: (path: string, sheet: string, headerRow?: number) =>
    invoke<SheetPreview>('import_preview', { path, sheet, headerRow: headerRow ?? null }),
  analyze: (input: {
    path: string
    sheet: string
    headerRow: number
    mapping: Mapping
    schoolYear: number
    defaultGrade: number | null
  }) => invoke<AnalyzeResponse>('import_analyze', { input }),
  apply: (analysisId: string, choice: ApplyChoice) =>
    invoke<ApplyStarted>('import_apply', { input: { analysisId, choice } }),
  history: (limit?: number) =>
    invoke<ImportHistoryRow[]>('import_history', { limit: limit ?? null }),
}

// ---------- 진행 상황 (설계안 §7.2) ----------

export interface JobProgress {
  jobId: string
  stage: string
  stageLabel: string
  done: number
  total: number
}

interface JobDone<T> {
  jobId: string
  result: T
}

interface JobError {
  jobId: string
  error: { code: string; userMessage: string; detail?: string | null }
}

/**
 * 한 작업의 진행·완료·실패를 듣는다. 돌려주는 함수를 부르면 그만 듣는다.
 *
 * 작업 번호가 다른 알림은 흘려보낸다 — 나중에 여러 작업이 함께 돌아도 섞이지 않는다.
 */
export function watchJob<T>(
  jobId: string,
  handlers: {
    onProgress?: (p: JobProgress) => void
    onDone?: (result: T) => void
    onError?: (error: unknown) => void
  },
): () => void {
  const stops: Array<() => void> = []
  let stopped = false

  const add = (p: Promise<() => void>) => {
    p.then((un) => {
      if (stopped) un()
      else stops.push(un)
    })
  }

  add(
    listen<JobProgress>('job://progress', (e) => {
      if (e.payload.jobId === jobId) handlers.onProgress?.(e.payload)
    }),
  )
  add(
    listen<JobDone<T>>('job://done', (e) => {
      if (e.payload.jobId === jobId) handlers.onDone?.(e.payload.result)
    }),
  )
  add(
    listen<JobError>('job://error', (e) => {
      if (e.payload.jobId === jobId) handlers.onError?.(e.payload.error)
    }),
  )

  return () => {
    stopped = true
    stops.forEach((un) => un())
  }
}
