import { invoke } from './invoke'

/** 백업을 왜 만들었는가. 자동 정리는 `AUTO` 만 건드린다. */
export type BackupKind =
  | 'AUTO'
  | 'MANUAL'
  | 'BEFORE_RESTORE'
  | 'BEFORE_MIGRATION'
  | 'BEFORE_TRANSITION'
  | 'OTHER'

export interface BackupEntry {
  fileName: string
  path: string
  kind: BackupKind
  kindLabel: string
  /** `2026.09.15. 18:50:00` */
  createdAt: string
  size: number
  /** 열어서 확인했는가 (integrity_check + 자료 구조) */
  ok: boolean
  schemaVersion: number | null
  /** 그 백업에 들어 있는 학생 수. 개인정보가 아니다 */
  students: number | null
  problem: string | null
  cleanedUp: boolean
}

export interface BackupStatus {
  folder: string
  total: number
  auto: number
  keptAuto: number
  latest: BackupEntry | null
  broken: number
  /** 다음 실행 때 되돌릴 파일이 놓여 있는가 */
  restoreWaiting: boolean
}

export interface RestoreStaged {
  fileName: string
  students: number | null
  schemaVersion: number | null
}

export const backupApi = {
  status: () => invoke<BackupStatus>('backup_status'),
  list: () => invoke<BackupEntry[]>('backup_list'),
  /** 지금 백업. 만든 뒤 바로 확인하고, 확인에 실패하면 그 파일을 지운다 */
  now: () => invoke<BackupEntry>('backup_now'),
  remove: (fileName: string) => invoke<void>('backup_delete', { fileName }),
  openFolder: () => invoke<void>('backup_open_folder'),
  /** 다른 곳에 보관한 파일이 이 프로그램의 성한 자료인지 본다 */
  checkFile: (path: string) => invoke<BackupEntry>('backup_check_file', { path }),
  /** 되돌릴 준비만 한다. 지금 자료는 프로그램을 다시 켤 때 바뀐다 */
  stageRestore: (path: string) => invoke<RestoreStaged>('restore_stage', { path }),
  cancelRestore: () => invoke<boolean>('restore_cancel'),
  integrity: () => invoke<string>('db_integrity'),
  quit: () => invoke<void>('app_quit'),
}

/** `1.2MB` */
export function fileSize(bytes: number): string {
  if (bytes >= 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)}MB`
  if (bytes >= 1024) return `${Math.round(bytes / 1024)}KB`
  return `${bytes}B`
}
