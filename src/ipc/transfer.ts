import { invoke } from './invoke'
import type { Gender, StudentInput } from './student'
// 인원 집계 타입은 통계 쪽 하나를 함께 쓴다 — 같은 숫자를 두 모양으로 두지 않는다.
import type { GradeCounts } from './stats'

export type { ClassCount, GradeCounts } from './stats'

/** 학년도별 학적 한 줄 — 같은 학생인지 사람이 보고 정하는 데 쓴다 */
export interface HistoryLine {
  schoolYear: number
  /** `2-나리 7번` */
  whereAt: string
  statusLabel: string
  /** `2026.05.14. 전출` 처럼 덧붙는 말 */
  extra: string | null
}

export interface StudentMatch {
  studentId: number
  name: string
  gender: Gender | null
  birthDate: string | null
  birthRaw: string | null
  history: HistoryLine[]
  /** 올해 이미 학적이 있으면 그 상태 이름 */
  currentStatus: string | null
}

export interface TransferInResult {
  studentId: number
  /** 학생을 새로 만들었는가 */
  created: boolean
  /** 같은 학년도 학적을 되살렸는가 */
  returned: boolean
  siblingCandidates: number
  numberDup: boolean
  studentLabel: string
}

export interface MoveRow {
  studentId: number
  name: string
  gender: Gender | null
  grade: number
  className: string | null
  classNo: number | null
  classLabel: string
  /** 전입생이면 전입일, 전출생이면 전출일 */
  date: string | null
  toSchool: string | null
  note: string | null
  status: string
  statusLabel: string
  issueCount: number
}

export const transferApi = {
  /** 기존 학생 찾기. 프로그램이 같은 학생이라고 정하지 않는다. */
  search: (name: string, birth: string | null, schoolYear: number) =>
    invoke<StudentMatch[]>('transfer_search_students', { name, birth, schoolYear }),

  /** 그 학년의 반별 인원. 반을 추천하지 않는다. */
  classCounts: (schoolYear: number, grade: number) =>
    invoke<GradeCounts>('transfer_class_counts', { schoolYear, grade }),

  usedNumbers: (schoolYear: number, grade: number, className: string | null) =>
    invoke<number[]>('transfer_used_numbers', { schoolYear, grade, className }),

  list: (schoolYear: number, wantIn: boolean, grade: number | null, q: string | null) =>
    invoke<MoveRow[]>('transfer_list', { schoolYear, wantIn, grade, q }),

  transferIn: (input: { studentId: number | null; student: StudentInput; date: string }) =>
    invoke<TransferInResult>('transfer_in', { input }),

  transferOut: (input: {
    studentId: number
    schoolYear: number
    date: string
    toSchool: string | null
    note: string | null
  }) => invoke<void>('transfer_out', { input }),

  cancelOut: (studentId: number, schoolYear: number) =>
    invoke<void>('transfer_out_cancel', { studentId, schoolYear }),

  pastOut: (input: {
    studentId: number | null
    student: StudentInput
    date: string
    toSchool: string | null
    note: string | null
  }) => invoke<number>('transfer_past_out', { input }),
}
