/**
 * 화면 표시용 순수 함수.
 *
 * 저장되는 값은 모두 Rust가 만든다. 여기 있는 것은 **보여 주기만** 하는 변환이라
 * 자료를 바꾸지 않는다. (연락처는 저장할 때 이미 정리되어 오므로 그대로 쓴다.)
 */
import type { EnrollStatus, Gender, StudentRow } from '@/ipc/student'
import type { Tone } from '@/components/ui'

/** `2017-03-15` → `17.03.15.` */
export function birthDisplay(iso: string | null | undefined): string {
  if (!iso) return ''
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso)
  if (!m) return iso
  return `${m[1].slice(2)}.${m[2]}.${m[3]}.`
}

/** 저장된 날짜가 없으면 원본을 그대로 보여 준다 — 무엇을 고쳐야 할지 알아야 하므로. */
export function birthCell(iso: string | null, raw: string | null): string {
  return iso ? birthDisplay(iso) : (raw ?? '')
}

export function genderLabel(g: Gender | null | undefined): string {
  return g === 'M' ? '남' : g === 'F' ? '여' : ''
}

/** 숫자만 남긴다. 검색 칸에서 Rust와 같은 규칙을 쓰기 위한 것. */
export function digitsOnly(s: string): string {
  return s.replace(/\D/g, '')
}

export interface StatusBadge {
  label: string
  tone: Tone
}

/**
 * 화면에 보여줄 상태.
 *
 * 졸업은 학년도별 상태가 아니라 한 번 있는 사건이라 학적과 따로 저장한다.
 * 그래서 표시할 때 둘을 합친다 — 그 해에 졸업했으면 졸업으로 보여 준다.
 */
export function statusBadge(row: {
  status: EnrollStatus
  graduated: boolean
}): StatusBadge {
  if (row.graduated) return { label: '졸업', tone: 'neutral' }
  switch (row.status) {
    case 'TRANSFER_IN':
      return { label: '전입', tone: 'info' }
    case 'TRANSFER_OUT':
      return { label: '전출', tone: 'error' }
    default:
      return { label: '재학', tone: 'success' }
  }
}

/** 확인 필요 표시. 0건이면 아무것도 보여 주지 않는다. */
export function issueBadge(row: Pick<StudentRow, 'issueCount'>): StatusBadge | null {
  if (row.issueCount <= 0) return null
  return { label: `확인 ${row.issueCount}`, tone: 'warn' }
}

/** `1,011명` */
export function countLabel(n: number): string {
  return `${n.toLocaleString('ko-KR')}명`
}

/** 쪽 나누기 안내 — `1,011명 중 1–100` */
export function rangeLabel(total: number, offset: number, shown: number): string {
  if (total === 0) return '0명'
  const from = offset + 1
  const to = offset + shown
  return `${total.toLocaleString('ko-KR')}명 중 ${from.toLocaleString('ko-KR')}–${to.toLocaleString('ko-KR')}`
}

/** `2026-05-14` → `2026.05.14.` 빈 값이면 빈 문자열 */
export function dotDate(iso: string | null | undefined): string {
  if (!iso) return ''
  const m = iso.match(/^(\d{4})-(\d{2})-(\d{2})/)
  return m ? `${m[1]}.${m[2]}.${m[3]}.` : iso
}
