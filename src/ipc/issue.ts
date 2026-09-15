import { invoke } from './invoke'

export interface IssueCount {
  kind: string
  kindLabel: string
  count: number
}

export interface IssueSummary {
  total: number
  byKind: IssueCount[]
}

/** 확인 필요를 추리는 조건. 기본은 현재 학년도 · 미해결. */
export interface IssueFilter {
  schoolYear: number
  grade?: number | null
  className?: string | null
  kind?: string | null
  /** OPEN(기본) / RESOLVED / ALL */
  status?: string | null
  /** 학생 이름이나 `3-나리 홍길동` 으로 찾기 */
  q?: string | null
  /** 전출한 학생의 표시까지 볼지. 기본은 false */
  includeLeft?: boolean
}

export type IssueStatus = 'OPEN' | 'RESOLVED' | 'DISMISSED'

export interface IssueListRow {
  id: number
  studentId: number
  /** `3-나리 홍길동` — 지금 학적으로 만든 이름표 */
  studentLabel: string
  classNo: number | null
  kind: string
  kindLabel: string
  message: string
  detail: string | null
  status: IssueStatus
  statusLabel: string
  /** 학생 상세의 어느 칸을 열지 */
  tab: string
  createdAt: string
  resolvedAt: string | null
}

export interface IssueListPage {
  rows: IssueListRow[]
  total: number
}

export const issueApi = {
  summary: (schoolYear: number) => invoke<IssueSummary>('issue_summary', { schoolYear }),
  /** 그 학년도 학생 전체의 확인 필요를 다시 따진다. 고친 학생 수를 돌려준다. */
  recompute: (schoolYear: number) => invoke<number>('issue_recompute', { schoolYear }),
  list: (filter: IssueFilter, limit: number, offset: number) =>
    invoke<IssueListPage>('issue_list', { filter, limit, offset }),
}
