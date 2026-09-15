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

export const issueApi = {
  summary: (schoolYear: number) => invoke<IssueSummary>('issue_summary', { schoolYear }),
  /** 그 학년도 학생 전체의 확인 필요를 다시 따진다. 고친 학생 수를 돌려준다. */
  recompute: (schoolYear: number) => invoke<number>('issue_recompute', { schoolYear }),
}
