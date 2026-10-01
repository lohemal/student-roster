import { useState } from 'react'
import { useMutation } from '@tanstack/react-query'
import { CalendarClock, RotateCcw } from 'lucide-react'

import { Badge, Button, ErrorNotice, Input } from '@/components/ui'
import { transferApi, type MoveRow } from '@/ipc/transfer'
import { dotDate } from '@/lib/format'
import s from './Transfer.module.css'

/**
 * 예정된 이동의 날짜를 고친다.
 *
 * **상태를 손으로 바꾸지 않는다.** 날짜만 고치면 현재 재학생 판정이 저절로 따라온다 —
 * 자정에 DB 를 손보는 장치가 없어도 다음에 여는 날 바로 맞는다.
 */
export function RescheduleButton({
  row,
  schoolYear,
  onDone,
}: {
  row: MoveRow
  schoolYear: number
  onDone: () => void
}) {
  const [editing, setEditing] = useState(false)
  const [date, setDate] = useState(row.date ?? '')

  const save = useMutation({
    mutationFn: () => transferApi.reschedule(row.studentId, schoolYear, date),
    onSuccess: () => {
      setEditing(false)
      onDone()
    },
  })

  if (!editing) {
    return (
      <Button
        size="sm"
        variant="ghost"
        icon={CalendarClock}
        onClick={(e) => {
          e.stopPropagation()
          setDate(row.date ?? '')
          setEditing(true)
        }}
      >
        예정일 변경
      </Button>
    )
  }

  return (
    <span className={s.pendingRow} onClick={(e) => e.stopPropagation()}>
      <Input
        type="date"
        className={s.dateEdit}
        value={date}
        onChange={(e) => setDate(e.target.value)}
      />
      <Button
        size="sm"
        variant="primary"
        disabled={save.isPending || !date}
        onClick={() => save.mutate()}
      >
        저장
      </Button>
      <Button size="sm" variant="ghost" onClick={() => setEditing(false)}>
        그만
      </Button>
      <ErrorNotice error={save.error} />
    </span>
  )
}

/** 전입 예정 되돌리기 */
export function CancelInButton({
  row,
  schoolYear,
  onDone,
}: {
  row: MoveRow
  schoolYear: number
  onDone: (message: string) => void
}) {
  const [asking, setAsking] = useState(false)

  const cancel = useMutation({
    mutationFn: () => transferApi.cancelIn(row.studentId, schoolYear),
    onSuccess: (r) => {
      setAsking(false)
      onDone(
        r.studentRemoved
          ? `${row.name} 학생의 전입 예정을 되돌리고 학생 자료도 지웠습니다. 이번 전입으로 처음 등록한 학생이었습니다.`
          : `${row.name} 학생의 ${schoolYear}학년도 전입 예정을 되돌렸습니다. 지난 학적은 그대로 남아 있습니다.`,
      )
    },
  })

  if (!asking) {
    return (
      <Button
        size="sm"
        variant="ghost"
        icon={RotateCcw}
        onClick={(e) => {
          e.stopPropagation()
          setAsking(true)
        }}
      >
        전입 취소
      </Button>
    )
  }

  return (
    <span className={s.pendingRow} onClick={(e) => e.stopPropagation()}>
      <span className={s.askText}>
        {dotDate(row.date)} 전입 예정을 되돌릴까요?
      </span>
      <Button
        size="sm"
        variant="danger"
        disabled={cancel.isPending}
        onClick={() => cancel.mutate()}
      >
        되돌리기
      </Button>
      <Button size="sm" variant="ghost" onClick={() => setAsking(false)}>
        그만
      </Button>
      <ErrorNotice error={cancel.error} />
    </span>
  )
}

/** 예정 이동 뱃지. 경고색이 아니라 상태 표시 정도로 둔다. */
export function PendingBadge({ row }: { row: MoveRow }) {
  if (!row.pending) return null
  return (
    <Badge tone="warn">
      {row.pendingLabel} · {dotDate(row.date)}
    </Badge>
  )
}
