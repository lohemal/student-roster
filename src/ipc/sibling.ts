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

export const siblingApi = {
  list: (studentId: number, schoolYear: number) =>
    invoke<SiblingView[]>('sibling_list', { studentId, schoolYear }),
  confirm: (linkId: number, schoolYear: number) =>
    invoke<void>('sibling_confirm', { linkId, schoolYear }),
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
