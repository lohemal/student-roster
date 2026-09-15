import { invoke } from './invoke'

/** 한 학년도가 지금 어떤 상태인가 */
export interface YearInfo {
  year: number
  exists: boolean
  /** 그 학년도 학적 수 (전출 포함) */
  enrollments: number
  active: number
}

/** 확인이 필요한 학생 하나 */
export interface Person {
  studentId: number | null
  /** `3-가람-7 홍길동` */
  label: string
  detail: string | null
}

export interface TargetInfo {
  from: YearInfo
  to: YearInfo
  /** 같은 전환을 이미 돌렸으면 그 시각 */
  doneAt: string | null
  /** 원본 학년도 6학년 재학생 — 기본 졸업 대상 */
  graduating: Person[]
  /** 배정이 필요한 1~5학년 수 */
  promoting: number
}

export interface AssignInfo {
  id: string
  fileName: string
  sheetName: string
  headerRow: number
  rows: number
  missing: string[]
}

/**
 * 막는 것과 알리는 것.
 *
 * 새 반이 없으면 학적을 만들 수 없으니 막고(`blocking`), 주소 미분류처럼 나중에
 * 고칠 수 있는 것은 막지 않는다.
 */
export type ProblemKind =
  | 'NO_ASSIGN'
  | 'NO_CLASS'
  | 'DUPLICATED'
  | 'UNKNOWN_STUDENT'
  | 'NAME_MISMATCH'
  | 'BAD_GRADE'
  | 'PARTIAL_NUMBERS'
  | 'NUMBER_DUP'
  | 'WRONG_YEAR'
  | 'STALE_SEAT'
  | 'UNUSUAL_GRADE'
  | 'NO_NEXT_ENROLLMENT'
  | 'TARGET_NOT_EMPTY'
  | 'ALREADY_DONE'

export interface Problem {
  kind: ProblemKind
  title: string
  message: string
  blocking: boolean
  students: Person[]
}

export interface PlanRow {
  studentId: number
  name: string
  /** `3-가람-7` */
  current: string
  /** `4-다솜-12` · `졸업` · `없음` */
  next: string
  fate: 'PROMOTE' | 'GRADUATE' | 'NONE'
}

export interface GradeMove {
  fromGrade: number
  toGrade: number | null
  label: string
  count: number
}

export interface ClassPreview {
  grade: number
  className: string | null
  classLabel: string
  male: number
  female: number
  unknown: number
  total: number
}

export interface Plan {
  fromYear: number
  toYear: number
  /** 전환 대상 (원본 학년도 재학생) */
  targetCount: number
  promoteCount: number
  graduateCount: number
  skipCount: number
  existingCount: number
  rows: PlanRow[]
  byGrade: GradeMove[]
  classes: ClassPreview[]
  problems: Problem[]
  /** 적용할 때 그대로 돌려준다. 그사이 명단이 바뀌면 거절된다 */
  stateKey: string
  blocked: boolean
  assignFile: string | null
  assignRows: number
}

export interface PlanInput {
  fromYear: number
  toYear: number
  /** 졸업에서 뺄 학생 */
  excludeGraduation?: number[]
}

export interface ApplyStarted {
  jobId: string
  total: number
  backupPath: string
  backupMs: number
}

export interface ApplyResult {
  fromYear: number
  toYear: number
  promoted: number
  graduated: number
  skipped: number
  issues: number
  backupPath: string
  elapsedMs: number
}

export interface TransitionHistory {
  id: number
  fromYear: number
  toYear: number
  executedAt: string
  summary: string | null
}

export const transitionApi = {
  target: (fromYear?: number, toYear?: number) =>
    invoke<TargetInfo>('transition_target', { fromYear, toYear }),
  /** 배정 파일을 읽어 둔다. 아직 아무것도 저장하지 않는다 */
  readAssign: (path: string, fromYear: number, sheet?: string) =>
    invoke<AssignInfo>('transition_read_assign', { path, fromYear, sheet }),
  clearAssign: () => invoke<void>('transition_clear_assign'),
  preview: (input: PlanInput) => invoke<Plan>('transition_preview', { input }),
  /** 백업을 만든 뒤 한 트랜잭션으로 진행한다. 끝은 `job://done` 으로 온다 */
  apply: (input: PlanInput & { stateKey: string }) =>
    invoke<ApplyStarted>('transition_apply', { input }),
  history: (limit?: number) => invoke<TransitionHistory[]>('transition_history', { limit }),
}
