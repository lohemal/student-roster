import { invoke } from './invoke'
import type { ListFilter } from './student'

/**
 * 어떤 양식으로 내보낼지.
 *
 * 셋은 **같은 엔진**을 쓴다. 다른 것은 열과 파일을 나누는 방법뿐이다.
 */
export type ExportPreset = 'CUSTOM' | 'SCHOOLJONGI' | 'ALIME'

/**
 * 뽑은 학생을 **몇 개 파일로 나눌지**.
 *
 * 누구를 뽑을지(`ListFilter`)와는 다른 이야기다. 3학년만 뽑아 반별로 나눌 수도 있고,
 * 5단지만 뽑아 학년별로 나눌 수도 있다.
 */
export type Grouping = 'ALL' | 'GRADE' | 'GRADE_CLASS'

/** 고를 수 있는 열. 이름과 차례의 원본은 Rust 에 있고 화면은 받아서 그린다. */
export interface ColumnInfo {
  key: string
  label: string
  /** 개인정보인가 */
  personal: boolean
  /** 처음 열었을 때 켜져 있는 열인가 */
  defaultOn: boolean
}

export interface SheetInfo {
  name: string
  students: number
}

export interface FileInfo {
  fileName: string
  label: string
  students: number
  sheets: SheetInfo[]
}

/** 확인이 필요한 학생 */
export interface ExportProblem {
  studentId: number
  /** `3-나리 홍길동` */
  label: string
}

export interface ExportWarning {
  message: string
  students: ExportProblem[]
  /** 이 학생들이 파일에서 빠지는가 */
  excluded: boolean
}

export interface ExportPreview {
  presetLabel: string
  groupingLabel: string
  headers: string[]
  /** 첫 몇 줄 — 실제로 파일에 들어갈 값 그대로 */
  sample: string[][]
  files: FileInfo[]
  /** 실제로 파일에 들어가는 학생 수 */
  students: number
  /** 조건에 든 학생 수 (빠지는 학생 포함) */
  matched: number
  warnings: ExportWarning[]
  /** 파일이 하나면 저장 창, 여럿이면 폴더 창을 연다 */
  singleFile: boolean
  defaultFileName: string | null
}

export interface ExportRequest {
  preset: ExportPreset
  /** 누구를 내보낼지 — 학생명단과 같은 조건 */
  filter: ListFilter
  /** 사용자 지정에서 고른 열. 고른 차례가 Excel 의 차례가 된다 */
  columns?: string[]
  grouping?: Grouping
}

export interface ExportResult {
  paths: string[]
  /** [폴더 열기] 에 쓴다 */
  folder: string
  files: number
  students: number
}

/** 같은 이름의 파일이 이미 있을 때 오는 오류 코드 */
export const EXPORT_EXISTS = 'EXPORT_EXISTS'

export const exportApi = {
  columns: () => invoke<ColumnInfo[]>('export_columns'),
  /** 만들어질 파일을 미리 본다. 아무것도 쓰지 않는다. */
  preview: (request: ExportRequest) => invoke<ExportPreview>('export_preview', { request }),
  /** `target` 은 파일이 하나면 파일 경로, 여럿이면 폴더 경로 */
  run: (request: ExportRequest, target: string, overwrite: boolean) =>
    invoke<ExportResult>('export_run', { request: { ...request, target, overwrite } }),
  openFolder: (path: string) => invoke<void>('export_open_folder', { path }),
}
