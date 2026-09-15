import { invoke } from './invoke'

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
  issueCount: number
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
}
