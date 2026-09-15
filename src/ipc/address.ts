import { invoke } from './invoke'

export type RuleKind = 'ROAD' | 'COMPLEX' | 'CONTAINS'

/** `students.address_source` 와 같은 값 */
export type AddressSource = 'NONE' | 'AUTO' | 'RULE' | 'MANUAL' | 'CONFLICT'

export interface CategoryRow {
  id: number
  name: string
  sortOrder: number
  isBuiltin: boolean
  studentCount: number
  ruleCount: number
}

export interface RuleRow {
  id: number
  kind: RuleKind
  kindLabel: string
  pattern: string
  categoryId: number
  categoryName: string
  isActive: boolean
  note: string | null
  applied: number
  createdAt: string
}

export interface Hit {
  ruleId: number
  kind: string
  pattern: string
  categoryId: number
  categoryName: string
}

export interface Suggestion {
  kind: RuleKind
  kindLabel: string
  pattern: string
  hint: string
}

export interface AddressStatus {
  categoryId: number | null
  categoryName: string | null
  source: AddressSource
  sourceLabel: string
  reason: string
  ruleId: number | null
  ruleText: string | null
  conflicts: Hit[]
  suggestions: Suggestion[]
}

/** 규칙 하나가 몇 명에게 걸리는지 */
export interface Matching {
  total: number
  unclassified: number
  already: number
  manual: number
}

export interface RuleCreated {
  ruleId: number
  matching: Matching
}

export interface ReapplyResult {
  total: number
  byRule: number
  byAuto: number
  keptManual: number
  unclassified: number
  conflict: number
  noAddress: number
  changed: number
}

export const RULE_KIND_LABEL: Record<RuleKind, string> = {
  ROAD: '도로명',
  COMPLEX: '단지명',
  CONTAINS: '포함',
}

export const RULE_KIND_HINT: Record<RuleKind, string> = {
  ROAD: '도로명과 건물번호가 정확히 같은 주소에만 적용됩니다. 동·호수는 달라도 됩니다.',
  COMPLEX: '주소에 이 단지 이름이 들어 있으면 적용됩니다. 띄어쓰기는 달라도 됩니다.',
  CONTAINS:
    '주소에 이 글자가 들어 있기만 하면 적용됩니다. 넓게 걸리므로 짧은 글자는 피해 주세요.',
}

export const addressApi = {
  // 분류
  categories: (schoolYear: number) =>
    invoke<CategoryRow[]>('address_category_list', { schoolYear }),
  createCategory: (name: string) => invoke<number>('address_category_create', { name }),
  renameCategory: (id: number, name: string) =>
    invoke<void>('address_category_rename', { id, name }),
  deleteCategory: (id: number) => invoke<void>('address_category_delete', { id }),

  // 규칙
  rules: (schoolYear: number) => invoke<RuleRow[]>('address_rule_list', { schoolYear }),
  createRule: (
    input: { kind: RuleKind; pattern: string; categoryId: number; note?: string | null },
    schoolYear: number,
  ) => invoke<RuleCreated>('address_rule_create', { input, schoolYear }),
  updateRule: (input: {
    id: number
    kind: RuleKind
    pattern: string
    categoryId: number
    isActive: boolean
  }) => invoke<void>('address_rule_update', { input }),
  deleteRule: (id: number) => invoke<void>('address_rule_delete', { id }),
  previewRule: (kind: RuleKind, pattern: string, schoolYear: number) =>
    invoke<Matching>('address_rule_preview', { kind, pattern, schoolYear }),
  applyRule: (ruleId: number, schoolYear: number) =>
    invoke<number>('address_rule_apply', { ruleId, schoolYear }),

  // 학생 한 명
  status: (studentId: number) => invoke<AddressStatus>('address_status', { studentId }),
  setManual: (studentId: number, categoryId: number | null, schoolYear: number) =>
    invoke<void>('address_set_manual', { studentId, categoryId, schoolYear }),

  // 전체 다시 적용 — 진행 상황은 job://progress 로 온다
  reapply: (schoolYear: number) =>
    invoke<{ jobId: string }>('address_reapply', { schoolYear }),
}
