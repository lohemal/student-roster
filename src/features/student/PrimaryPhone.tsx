import { useState } from 'react'
import { useMutation, useQuery } from '@tanstack/react-query'
import { Phone } from 'lucide-react'

import { Badge, Button, ErrorNotice, Notice } from '@/components/ui'
import {
  studentApi,
  type ListFilter,
  type PrimarySource,
  type StudentRow,
} from '@/ipc/student'
import s from './PrimaryPhone.module.css'

const n = (v: number) => v.toLocaleString('ko-KR')

/** 색만으로 뜻을 전하지 않는다 — 언제나 `모`·`부` 글자가 함께 있다 */
const TONE: Record<PrimarySource, 'info' | 'success' | 'neutral'> = {
  MOTHER: 'success',
  FATHER: 'info',
  BOTH: 'info',
  OTHER: 'neutral',
  NONE: 'neutral',
}

/**
 * 주보호자 연락처와 그 출처.
 *
 * 출처는 저장하지 않는다 — 부·모 연락처와 **견주어** 그때그때 가린다. 모 연락처를
 * 고치면 `[모]` 가 사라지는데, 그것이 사실이다(그 번호는 더 이상 모의 번호가 아니다).
 */
export function PrimaryPhoneCell({
  row,
}: {
  row: Pick<StudentRow, 'primaryPhone' | 'primarySource' | 'primarySourceLabel'>
}) {
  if (!row.primaryPhone) return null
  return (
    <span className={s.cell}>
      <span className={s.number}>{row.primaryPhone}</span>
      {row.primarySourceLabel && (
        <Badge tone={TONE[row.primarySource]}>{row.primarySourceLabel}</Badge>
      )}
    </span>
  )
}

interface Props {
  /** 지금 화면에 걸려 있는 조건 */
  filter: ListFilter
  /** 조건에 걸린 인원 — 범위를 고를 때 보여 준다 */
  filtered: number
  /** 조건이 하나라도 걸려 있는가 */
  hasFilter: boolean
  onClose: () => void
  onDone: (message: string) => void
}

type Scope = 'FILTER' | 'ALL'
type From = 'MOTHER' | 'FATHER'

/**
 * 주보호자 연락처 일괄 설정.
 *
 * **덮어쓰는 작업이므로 먼저 세어 보여 준다.** 몇 명이 새로 채워지고 몇 명이
 * 덮어써지는지, 가져올 연락처가 없어 그대로 두는 학생이 몇 명인지까지 보고 나서
 * 사람이 누른다. 미리보기와 실행은 같은 명령·같은 조건을 쓴다.
 */
export function PrimaryFillDialog({ filter, filtered, hasFilter, onClose, onDone }: Props) {
  const [scope, setScope] = useState<Scope>(hasFilter ? 'FILTER' : 'ALL')
  const [from, setFrom] = useState<From>('MOTHER')

  /** 전체 재학생은 학년도만 남긴 조건이다 */
  const target: ListFilter =
    scope === 'ALL' ? { schoolYear: filter.schoolYear } : filter

  const preview = useQuery({
    queryKey: ['primary-fill', target, from],
    queryFn: () => studentApi.primaryFill(target, from, false),
  })

  const apply = useMutation({
    mutationFn: () => studentApi.primaryFill(target, from, true),
    onSuccess: (r) =>
      onDone(
        `${n(r.changed)}명의 주보호자 연락처를 ${r.fromLabel}로 맞췄습니다.` +
          (r.missing > 0
            ? ` ${r.fromLabel}가 없는 ${n(r.missing)}명은 그대로 두었습니다.`
            : ''),
      ),
  })

  const p = preview.data
  const willChange = p ? p.fillEmpty + p.overwrite : 0

  return (
    <div className={s.overlay} onClick={() => !apply.isPending && onClose()}>
      <div className={s.box} role="dialog" aria-modal="true" onClick={(e) => e.stopPropagation()}>
        <header className={s.head}>
          <h2 className={s.title}>주보호자 연락처 일괄 설정</h2>
          <p className={s.sub}>
            학생마다 하나씩 넣지 않고, 고른 범위의 주보호자 연락처를 부·모 연락처로
            한꺼번에 맞춥니다.
          </p>
        </header>

        <div className={s.body}>
          <ErrorNotice error={preview.error ?? apply.error} />

          <div className={s.group}>
            <div className={s.groupTitle}>무엇으로 맞출까요</div>
            <div className={s.choices}>
              <Choice on={from === 'MOTHER'} onClick={() => setFrom('MOTHER')}>
                모 연락처로
              </Choice>
              <Choice on={from === 'FATHER'} onClick={() => setFrom('FATHER')}>
                부 연락처로
              </Choice>
            </div>
          </div>

          <div className={s.group}>
            <div className={s.groupTitle}>누구에게 적용할까요</div>
            <div className={s.choices}>
              <Choice
                on={scope === 'FILTER'}
                disabled={!hasFilter}
                onClick={() => setScope('FILTER')}
              >
                지금 조건에 보이는 학생
                <span className={s.choiceNum}>{n(filtered)}명</span>
              </Choice>
              <Choice on={scope === 'ALL'} onClick={() => setScope('ALL')}>
                올해 현재 재학생 전체
              </Choice>
            </div>
            {!hasFilter && (
              <p className={s.hint}>
                지금은 조건을 걸지 않아 둘이 같습니다. 학년·반 등으로 추린 뒤 다시 열면
                그 학생들에게만 적용할 수 있습니다.
              </p>
            )}
          </div>

          {p ? (
            <>
              <div className={s.plan}>
                <PlanLine label="대상 학생" value={p.target} strong />
                <PlanLine label={`${p.fromLabel} 있음 · 비어 있어 채움`} value={p.fillEmpty} />
                <PlanLine
                  label={`${p.fromLabel} 있음 · 다른 번호를 덮어씀`}
                  value={p.overwrite}
                  warn={p.overwrite > 0}
                />
                <PlanLine label="이미 같은 번호" value={p.already} muted />
                <PlanLine
                  label={`${p.fromLabel} 없음 · 그대로 둠`}
                  value={p.missing}
                  muted
                />
              </div>

              {p.overwrite > 0 && (
                <Notice tone="warn">
                  <b>{n(p.overwrite)}명은 이미 다른 번호가 들어 있습니다.</b> 실행하면 그
                  값은 {p.fromLabel}로 바뀝니다. 되돌리려면 학생마다 다시 입력해야 하니
                  범위를 한 번 더 확인해 주세요.
                </Notice>
              )}
              {p.missing > 0 && (
                <Notice tone="info">
                  {p.fromLabel}가 없는 {n(p.missing)}명은 <b>건드리지 않습니다.</b> 빈 값으로
                  덮어쓰면 그 학생이 문자 명단에서 조용히 빠지기 때문입니다.
                </Notice>
              )}
            </>
          ) : (
            <div className={s.hint}>세어 보는 중…</div>
          )}
        </div>

        <footer className={s.foot}>
          <span className={s.spacer} />
          <Button variant="outline" onClick={onClose} disabled={apply.isPending}>
            취소
          </Button>
          <Button
            variant="primary"
            icon={Phone}
            disabled={apply.isPending || !p || willChange === 0}
            onClick={() => apply.mutate()}
          >
            {apply.isPending
              ? '맞추는 중…'
              : willChange === 0
                ? '바뀔 학생이 없습니다'
                : `${n(willChange)}명 적용`}
          </Button>
        </footer>
      </div>
    </div>
  )
}

function Choice({
  on,
  disabled,
  onClick,
  children,
}: {
  on: boolean
  disabled?: boolean
  onClick: () => void
  children: React.ReactNode
}) {
  return (
    <button
      type="button"
      className={on ? `${s.choice} ${s.choiceOn}` : s.choice}
      disabled={disabled}
      onClick={onClick}
    >
      {children}
    </button>
  )
}

function PlanLine({
  label,
  value,
  strong,
  muted,
  warn,
}: {
  label: string
  value: number
  strong?: boolean
  muted?: boolean
  warn?: boolean
}) {
  const cls = [s.planLine, strong && s.planStrong, muted && s.planMuted, warn && s.planWarn]
    .filter(Boolean)
    .join(' ')
  return (
    <div className={cls}>
      <span>{label}</span>
      <span className={s.planNum}>{n(value)}명</span>
    </div>
  )
}
