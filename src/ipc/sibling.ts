import { invoke } from './invoke'

/** 견주는 네 항목 */
export type GuardianField = 'fatherName' | 'motherName' | 'fatherPhone' | 'motherPhone'

export type SiblingStatus = 'CANDIDATE' | 'CONFIRMED' | 'REJECTED'

export interface FieldView {
  key: GuardianField
  label: string
  /** 이 학생의 값 */
  mine: string | null
  /** 형제의 값 */
  theirs: string | null
}

export interface SiblingView {
  linkId: number
  studentId: number
  /** `1-나리 홍길동` — 지금 학적으로 만든 이름표 */
  label: string
  status: SiblingStatus
  statusLabel: string
  matched: FieldView[]
  conflicts: FieldView[]
  /** 형제에게는 있고 나에게는 없는 항목 */
  fillable: FieldView[]
  /** 지금 함께 다니지 않으면 그 까닭 (전출 / 지난 학년도) */
  partnerNote: string | null
  foundAt: string
  decidedAt: string | null
}

/** 학생명단에 보여 줄 짧은 형제 표시 */
export interface SiblingBrief {
  count: number
  text: string
}

export interface ScanResult {
  scanned: number
  newCandidates: number
  existingCandidates: number
  confirmed: number
  rejected: number
  dropped: number
  guardianFill: number
  guardianConflict: number
  skippedValues: number
}

/** 확인 필요 화면에 떠 있는 형제 후보 한 쌍 */
export interface CandidateRow {
  linkId: number
  studentA: number
  labelA: string
  studentB: number
  labelB: string
  /** 후보로 본 근거 — `부 성명`, `모 연락처` */
  matched: string[]
  /** 양쪽 다 값이 있는데 서로 다른 항목 */
  conflicts: string[]
  foundAt: string
}

/** 일괄 확정 결과 */
export interface BatchConfirm {
  confirmed: number
  already: number
  /** '형제 아님' 으로 정해 둔 것이라 건드리지 않은 수 */
  skipped: number
  students: number[]
}

export const siblingApi = {
  list: (studentId: number, schoolYear: number) =>
    invoke<SiblingView[]>('sibling_list', { studentId, schoolYear }),
  confirm: (linkId: number, schoolYear: number) =>
    invoke<void>('sibling_confirm', { linkId, schoolYear }),
  /** 확인 필요 화면의 형제 후보를 쌍마다 한 줄로 */
  candidates: (schoolYear: number) =>
    invoke<CandidateRow[]>('sibling_candidates', { schoolYear }),
  /** 고른 후보를 한 트랜잭션에서 확정한다 — 절반만 확정되지 않는다 */
  confirmMany: (linkIds: number[], schoolYear: number) =>
    invoke<BatchConfirm>('sibling_confirm_many', { linkIds, schoolYear }),
  reject: (linkId: number, schoolYear: number) =>
    invoke<void>('sibling_reject', { linkId, schoolYear }),
  reset: (linkId: number, schoolYear: number) =>
    invoke<void>('sibling_reset', { linkId, schoolYear }),
  fill: (input: {
    linkId: number
    studentId: number
    fields: GuardianField[]
    schoolYear: number
  }) => invoke<{ filled: string[] }>('sibling_fill', { input }),
  /** 전체 다시 찾기 — 진행 상황은 job://progress 로 온다 */
  rescan: (schoolYear: number) => invoke<{ jobId: string }>('sibling_rescan', { schoolYear }),
}
