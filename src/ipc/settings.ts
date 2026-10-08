import { invoke } from './invoke'

export interface SchoolYearRow {
  year: number
  isCurrent: boolean
  studentCount: number
  createdAt: string
}

export interface SettingsView {
  schoolName: string
  currentYear: number | null
  years: SchoolYearRow[]
  dbPath: string
}

/**
 * 학년도를 지우면 무엇이 사라지고 무엇이 남는가.
 *
 * **실제로 셀 수 있는 숫자만 온다.** 화면은 0 인 항목을 그리지 않는다 —
 * 없는 것을 0 으로 보여 주면 "이것도 지워지나" 하고 읽게 된다.
 */
export interface YearDeleteImpact {
  year: number
  deletable: boolean
  /** 막혔으면 그 까닭 */
  refusalCode: string | null
  refusalMessage: string | null

  // 지워지는 것
  enrollments: number
  events: number
  moves: number
  renumberOps: number
  imports: number
  transitions: number
  // 다시 계산되는 것
  issues: number
  // 남는 것
  students: number
  studentsWithoutOtherYear: number
  otherEnrollments: number
  graduations: number
  graduationsFromTransition: number
  graduationYear: number | null
  siblingLinks: number
}

export interface YearDeleteResult {
  year: number
  enrollments: number
  events: number
  renumberOps: number
  imports: number
  transitions: number
  resynced: number
  /** 지우기 직전에 만들어 둔 백업 파일 */
  backupPath: string
}

export const settingsApi = {
  get: () => invoke<SettingsView>('settings_get'),
  saveSchool: (schoolName: string) =>
    invoke<void>('settings_save_school', { input: { schoolName } }),
  createYear: (year: number, setCurrent: boolean) =>
    invoke<void>('school_year_create', { input: { year, setCurrent } }),
  setCurrentYear: (year: number) => invoke<void>('school_year_set_current', { year }),

  /** 삭제 전 영향 범위. 아무것도 바꾸지 않는다. */
  yearDeleteImpact: (year: number) =>
    invoke<YearDeleteImpact>('school_year_delete_impact', { year }),

  /** 학년도를 지운다. 백업이 성공한 뒤에만 시작한다. */
  deleteYear: (year: number) => invoke<YearDeleteResult>('school_year_delete', { year }),
}
