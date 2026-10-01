import { invoke } from './invoke'
import type { SiblingBrief } from './sibling'

export type Gender = 'M' | 'F'
export type EnrollStatus = 'ENROLLED' | 'TRANSFER_IN' | 'TRANSFER_OUT'

/** 학생명단 한 줄 */
export interface StudentRow {
  id: number
  name: string
  gender: Gender | null
  birthDate: string | null
  birthRaw: string | null
  grade: number
  className: string | null
  classNo: number | null
  classLabel: string
  status: EnrollStatus
  graduated: boolean
  addressRaw: string | null
  addressCategory: string | null
  fatherPhone: string | null
  motherPhone: string | null
  primaryPhone: string | null
  /** 주보호자 연락처가 어디서 온 번호인가 (저장값이 아니라 볼 때마다 견준다) */
  primarySource: PrimarySource
  /** `모` / `부` / `모·부` / `기타`. 없으면 null */
  primarySourceLabel: string | null
  issueCount: number
  /** 아직 오지 않은 이동이면 `IN` / `OUT` */
  pending: 'IN' | 'OUT' | null
  /** `전입 예정 · 2026.10.05.` */
  pendingLabel: string | null
  /** 본교 형제 — 없으면 null */
  sibling: SiblingBrief | null
}

/** 주보호자 연락처의 출처. 부·모 연락처와 견주어 가린다. */
export type PrimarySource = 'MOTHER' | 'FATHER' | 'BOTH' | 'OTHER' | 'NONE'

/** 주보호자 연락처 일괄 설정이 무슨 일을 하는가 */
export interface PrimaryFillPlan {
  /** `모 연락처` / `부 연락처` */
  fromLabel: string
  /** 조건에 든 학생 수 */
  target: number
  /** 주보호자 칸이 비어 있어 새로 채울 학생 */
  fillEmpty: number
  /** 다른 번호가 들어 있어 덮어쓸 학생 */
  overwrite: number
  /** 이미 같은 번호라 할 일이 없는 학생 */
  already: number
  /** 가져올 연락처가 없어 건드리지 않는 학생 */
  missing: number
  /** 실제로 바꾼 학생 수. 미리보기는 0 */
  changed: number
}

export interface EnrollmentRow {
  schoolYear: number
  grade: number
  className: string | null
  classNo: number | null
  classLabel: string
  status: EnrollStatus
  transferInDate: string | null
  transferOutDate: string | null
  transferOutNote: string | null
  graduated: boolean
  /** 아직 오지 않은 이동이면 `전입 예정 · 2026.10.05.` */
  pendingLabel: string | null
}

export interface EventRow {
  id: number
  schoolYear: number
  kind: string
  kindLabel: string
  eventDate: string | null
  classLabel: string | null
  classNo: number | null
  note: string | null
  createdAt: string
}

export interface IssueRow {
  id: number
  kind: string
  kindLabel: string
  message: string
  detail: string | null
  refId: number | null
  createdAt: string
}

export interface StudentDetail {
  id: number
  name: string
  gender: Gender | null
  birthRaw: string | null
  birthDate: string | null
  addressRaw: string | null
  addressCategoryId: number | null
  addressCategory: string | null
  addressSource: string
  fatherName: string | null
  motherName: string | null
  fatherPhone: string | null
  motherPhone: string | null
  primaryPhone: string | null
  note: string | null
  createdAt: string
  updatedAt: string
  enrollment: EnrollmentRow | null
  enrollments: EnrollmentRow[]
  events: EventRow[]
  issues: IssueRow[]
}

/** 저장할 때 보내는 값. 비어 있는 칸은 빈 문자열로 보내면 Rust가 비움 처리한다. */
export interface StudentInput {
  name: string
  gender: Gender | '' | null
  birthRaw: string
  addressRaw: string
  fatherName: string
  motherName: string
  fatherPhone: string
  motherPhone: string
  primaryPhone: string
  note: string
  schoolYear: number
  grade: number
  className: string
  classNo: number | null
  /** 주소를 바꿀 때 직접 지정한 주소 분류를 그대로 둘지 */
  keepManualAddress?: boolean
}

export type StatusFilter = 'ACTIVE' | 'ALL' | 'ENROLLED' | 'TRANSFER_IN' | 'TRANSFER_OUT'

export interface ListFilter {
  schoolYear: number
  q?: string
  grade?: number | null
  className?: string | null
  addressCategoryId?: number | null
  /** 주소는 있는데 아직 분류하지 못한 학생만 */
  addressUnclassified?: boolean
  /** 주소 자체가 없는 학생만. 통계의 [주소 없음] 에서 넘어올 때 쓴다 */
  addressNone?: boolean
  status?: StatusFilter
  name?: string
  classNo?: number | null
  address?: string
  fatherPhone?: string
  motherPhone?: string
  primaryPhone?: string
  onlyIssues?: boolean
}

export interface ListResult {
  rows: StudentRow[]
  total: number
  limit: number
  offset: number
}

export interface ClassOption {
  grade: number
  className: string
  count: number
}

export const studentApi = {
  list: (filter: ListFilter, limit: number, offset: number) =>
    invoke<ListResult>('student_list', { query: { ...filter, limit, offset } }),
  get: (id: number, schoolYear: number) =>
    invoke<StudentDetail>('student_get', { id, schoolYear }),
  create: (input: StudentInput) => invoke<number>('student_create', { input }),
  update: (id: number, input: StudentInput) => invoke<void>('student_update', { id, input }),
  remove: (id: number) => invoke<void>('student_delete', { id, confirm: true }),
  classOptions: (schoolYear: number) =>
    invoke<ClassOption[]>('student_class_options', { schoolYear }),
  /**
   * 주보호자 연락처를 모·부 연락처로 한꺼번에 맞춘다.
   *
   * `apply = false` 로 먼저 세어 보여 주고, 사람이 정한 뒤 같은 조건으로 다시 부른다.
   */
  primaryFill: (filter: ListFilter, from: 'MOTHER' | 'FATHER', apply: boolean) =>
    invoke<PrimaryFillPlan>('student_primary_fill', { input: { filter, from, apply } }),
}
