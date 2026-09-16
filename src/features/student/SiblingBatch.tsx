import { useMemo, useState } from 'react'
import { useMutation, useQuery } from '@tanstack/react-query'
import { AlertTriangle, Check, Users } from 'lucide-react'

import { Badge, Button, ErrorNotice, Notice } from '@/components/ui'
import { siblingApi, type CandidateRow } from '@/ipc/sibling'
import s from './SiblingBatch.module.css'

const n = (v: number) => v.toLocaleString('ko-KR')

interface Props {
  schoolYear: number
  /** 확정한 뒤 확인 필요 목록·뱃지를 다시 읽는다 */
  onChanged: () => void
}

/**
 * 형제 후보 일괄 확정.
 *
 * 후보를 저절로 확정하지 않는다 — **사람이 목록을 보고 한 번에 정할 뿐**이다.
 * 판정 규칙(부·모 성명과 연락처 가운데 둘 이상 일치)은 그대로 두고, 후보마다
 * 학생 상세를 열던 일만 없앤다.
 *
 * 확정은 명령 하나로 보낸다. 쉰 건을 쉰 번 부르면 중간에 멈췄을 때 절반만
 * 형제가 되고, 화면은 어디까지 끝났는지 알 수 없다.
 */
export function SiblingBatch({ schoolYear, onChanged }: Props) {
  const list = useQuery({
    queryKey: ['sibling-candidates', schoolYear],
    queryFn: () => siblingApi.candidates(schoolYear),
  })

  /** 고른 관계 번호. 목록을 처음 받으면 전부 고른 상태로 둔다. */
  const [off, setOff] = useState<Set<number>>(new Set())
  const [asking, setAsking] = useState(false)
  const [done, setDone] = useState<string | null>(null)

  const rows = useMemo(() => list.data ?? [], [list.data])
  const picked = useMemo(
    () => rows.filter((r) => !off.has(r.linkId)),
    [rows, off],
  )

  const confirm = useMutation({
    mutationFn: () =>
      siblingApi.confirmMany(
        picked.map((r) => r.linkId),
        schoolYear,
      ),
    onSuccess: (r) => {
      setAsking(false)
      setOff(new Set())
      const parts = [`${n(r.confirmed)}건을 형제로 확정했습니다.`]
      if (r.already > 0) parts.push(`${n(r.already)}건은 이미 확정되어 있었습니다.`)
      if (r.skipped > 0)
        parts.push(`${n(r.skipped)}건은 '형제 아님' 으로 정해 두어 그대로 두었습니다.`)
      setDone(parts.join(' '))
      list.refetch()
      onChanged()
    },
    onError: () => setAsking(false),
  })

  if (list.isLoading) return <div className={s.loading}>형제 후보를 불러오는 중…</div>

  if (rows.length === 0) {
    return (
      <div className={s.wrap}>
        {done && <Notice tone="success">{done}</Notice>}
        {!done && (
          <div className={s.loading}>
            <Check size={15} /> 확정할 형제 후보가 없습니다.
          </div>
        )}
      </div>
    )
  }

  const allOn = off.size === 0
  const toggle = (linkId: number) =>
    setOff((prev) => {
      const next = new Set(prev)
      if (next.has(linkId)) next.delete(linkId)
      else next.add(linkId)
      return next
    })

  return (
    <div className={s.wrap}>
      <ErrorNotice error={confirm.error} />
      {done && <Notice tone="success">{done}</Notice>}

      <div className={s.head}>
        <Users size={16} className={s.headIcon} />
        <span className={s.headTitle}>형제 후보 {n(rows.length)}건</span>
        <span className={s.headHint}>
          부·모 성명과 연락처 가운데 둘 이상이 같은 학생끼리 묶은 것입니다. 아닌 쌍은
          체크를 풀어 주세요.
        </span>
        <span className={s.spacer} />
        <Button
          size="sm"
          variant="outline"
          onClick={() => setOff(allOn ? new Set(rows.map((r) => r.linkId)) : new Set())}
        >
          {allOn ? '전체 선택 해제' : '전체 선택'}
        </Button>
        <Button
          size="sm"
          variant="primary"
          icon={Check}
          disabled={picked.length === 0 || confirm.isPending}
          onClick={() => setAsking(true)}
        >
          {confirm.isPending
            ? '확정하는 중…'
            : `선택한 ${n(picked.length)}건 형제로 확정`}
        </Button>
      </div>

      {asking && (
        <Notice tone="warn">
          <strong>형제 후보 {n(picked.length)}건을 모두 형제로 확정합니다.</strong>
          <div className={s.askBody}>
            부·모 성명 및 연락처 가운데 둘 이상이 일치하여 형제 후보로 판정된 학생들입니다.
            확정하면 형제 관계로 등록되고, 보호자 정보 보완·불일치 확인이 이어서
            계산됩니다. 확정한 뒤에도 학생 상세에서 한 건씩 되돌릴 수 있습니다.
          </div>
          <div className={s.askRow}>
            <Button
              variant="primary"
              disabled={confirm.isPending}
              onClick={() => confirm.mutate()}
            >
              {n(picked.length)}건 모두 확정
            </Button>
            <Button variant="ghost" onClick={() => setAsking(false)}>
              취소
            </Button>
          </div>
        </Notice>
      )}

      <ul className={s.list}>
        {rows.map((r) => (
          <Item
            key={r.linkId}
            row={r}
            on={!off.has(r.linkId)}
            onToggle={() => toggle(r.linkId)}
          />
        ))}
      </ul>
    </div>
  )
}

function Item({
  row,
  on,
  onToggle,
}: {
  row: CandidateRow
  on: boolean
  onToggle: () => void
}) {
  return (
    <li className={on ? `${s.item} ${s.itemOn}` : s.item}>
      <label className={s.pick}>
        <input type="checkbox" checked={on} onChange={onToggle} />
        <span className={s.pair}>
          <b>{row.labelA}</b>
          <span className={s.arrow}>↔</span>
          <b>{row.labelB}</b>
        </span>
      </label>
      <span className={s.why}>
        {row.matched.map((m) => (
          <Badge key={m} tone="info">
            {m} 일치
          </Badge>
        ))}
        {row.conflicts.length > 0 && (
          <span className={s.conflict} title="형제라도 값이 다를 수 있습니다">
            <AlertTriangle size={13} /> {row.conflicts.join(' · ')} 다름
          </span>
        )}
      </span>
    </li>
  )
}
