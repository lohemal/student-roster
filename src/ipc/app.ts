import { invoke } from './invoke'

/** 프로그램을 켤 때 있었던 일. 개인정보는 담기지 않는다 */
export interface StartupNote {
  /** RESTORED · RESTORE_REJECTED · INTEGRITY · BACKUP_FAILED */
  kind: string
  tone: 'success' | 'warn' | 'error' | 'info'
  message: string
}

export interface AppInfo {
  appVersion: string
  schemaVersion: number
  latestSchemaVersion: number
  dbPath: string
  backupDir: string
  setupCompleted: boolean
  notes: StartupNote[]
}

export const appApi = {
  info: () => invoke<AppInfo>('app_info'),
  openDataFolder: () => invoke<void>('data_open_folder'),
  /** 자동 확인은 하루 한 번이면 충분하다 */
  updateCheckDue: () => invoke<boolean>('update_check_due'),
  updateMarkChecked: () => invoke<void>('update_mark_checked'),
}
