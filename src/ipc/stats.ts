import { invoke } from './invoke'

/** 남 + 여 + 미입력 = 합계. 어느 표에서나 이 셋이 맞아야 한다. */
export interface Counts {
  male: number
  female: number
  /** 성별이 비어 있는 학생 — 남·여 어느 쪽에도 넣지 않는다 */
  unknown: number
  total: number
}

export interface GradeRow extends Counts {
  grade: number
}

export interface ClassCount extends Counts {
  grade: number
  className: string | null
  /** `3-나리` / 반이 없으면 `3-미정` */
  classLabel: string
}

/** 한 학년의 반별 인원 + 그 학년 합계. 전입 반 배정 화면이 쓴다. */
export interface GradeCounts {
  grade: number
  classes: ClassCount[]
  total: ClassCount
}

/** 주소 표의 한 줄이 무엇을 가리키는지 */
export type AddressBucket = 'CATEGORY' | 'UNCLASSIFIED' | 'NO_ADDRESS'

export interface AddressRow {
  bucket: AddressBucket
  /** 분류일 때만 값이 있다 */
  categoryId: number | null
  name: string
  /** `AddressTable.grades` 와 같은 차례 */
  byGrade: number[]
  total: number
}

export interface AddressTable {
  grades: number[]
  rows: AddressRow[]
  gradeTotals: number[]
  total: number
}

export interface AddressQuality {
  classified: number
  unclassified: number
  noAddress: number
  manual: number
  rule: number
  auto: number
  conflict: number
  total: number
}

export interface Consistency {
  total: number
  genderSum: number
  gradeSum: number
  classSum: number
  addressSum: number
  crossSum: number
  ok: boolean
}

/**
 * 어느 학생을 셀지.
 *
 * 화면의 모든 표가 이 하나를 함께 쓴다. 앞으로 내보내기도 같은 조건을 받는다.
 */
export interface StatFilter {
  schoolYear: number
  grade?: number | null
  addressCategoryId?: number | null
  /** 주소는 있는데 아직 분류하지 못한 학생만 */
  addressUnclassified?: boolean
  /** 주소 자체가 없는 학생만 */
  addressNone?: boolean
}

export interface StatsOverview {
  schoolYear: number
  /** 지금 학년도가 아니면 '학년도 최종 재적 기준' 이라고 알려 준다 */
  isCurrentYear: boolean
  totals: Counts
  byGrade: GradeRow[]
  byClass: ClassCount[]
  address: AddressTable
  addressQuality: AddressQuality
  consistency: Consistency
}

export const statsApi = {
  /** 고른 조건으로 모든 표를 한 번에 센다. */
  overview: (filter: StatFilter) => invoke<StatsOverview>('stats_overview', { filter }),
}
