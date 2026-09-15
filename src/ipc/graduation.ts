import { invoke } from './invoke'

export interface GradYear {
  schoolYear: number
  count: number
}

/** 졸업생 한 명 — 졸업 당시 자리 그대로 */
export interface GradRow {
  studentId: number
  schoolYear: number
  name: string
  gender: string | null
  birthDate: string | null
  birthRaw: string | null
  grade: number | null
  className: string | null
  classNo: number | null
  /** `6-가람` */
  classLabel: string | null
  graduatedAt: string | null
  note: string | null
}

export const graduationApi = {
  years: () => invoke<GradYear[]>('graduation_years'),
  list: (schoolYear: number, q?: string) =>
    invoke<GradRow[]>('graduation_list', { schoolYear, q }),
  /** 잘못 만든 졸업 기록을 되돌린다. 돌아온 학년도를 준다 */
  cancel: (studentId: number) => invoke<number>('graduation_cancel', { studentId }),
}
