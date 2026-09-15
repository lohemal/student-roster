import { invoke } from './invoke'

/** 무엇을 하는 계획인지 */
export type RenumberKind = 'NONE' | 'ASSIGN' | 'REORDER' | 'INSERT'

export interface RenumberRow {
  studentId: number
  name: string
  /** 번호가 없던 학생이면 null */
  from: number | null
  to: number
  /** 사용자가 직접 고른 학생인지 */
  isTarget: boolean
}

export interface RenumberPreview {
  studentId: number
  name: string
  classLabel: string
  from: number | null
  to: number
  kind: RenumberKind
  /** 사용자에게 보여 줄 한 줄 설명 */
  summary: string
  /** 번호가 바뀌는 학생만. 대상 학생도 들어 있다. */
  rows: RenumberRow[]
  /** 대상을 뺀, 덩달아 바뀌는 인원 */
  affected: number
  /** 저장할 때 명단이 그대로인지 확인하는 값 */
  stateKey: string
  /** 할 수 없는 경우 그 까닭 */
  blocked: string | null
}

export interface RenumberResult {
  moved: number
  classLabel: string
  opId: number | null
}

export const renumberApi = {
  /** 무슨 일이 생기는지 미리 계산한다. 저장하지 않는다. */
  preview: (studentId: number, schoolYear: number, newNo: number) =>
    invoke<RenumberPreview>('renumber_preview', { studentId, schoolYear, newNo }),
  /** 미리 보여 준 계획을 저장한다. */
  apply: (input: {
    studentId: number
    schoolYear: number
    newNo: number
    stateKey: string
  }) => invoke<RenumberResult>('renumber_apply', { input }),
}
