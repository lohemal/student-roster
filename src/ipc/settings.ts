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

export const settingsApi = {
  get: () => invoke<SettingsView>('settings_get'),
  saveSchool: (schoolName: string) =>
    invoke<void>('settings_save_school', { input: { schoolName } }),
  createYear: (year: number, setCurrent: boolean) =>
    invoke<void>('school_year_create', { input: { year, setCurrent } }),
  setCurrentYear: (year: number) => invoke<void>('school_year_set_current', { year }),
}
