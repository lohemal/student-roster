import { useEffect, useState } from 'react'
import { useMutation } from '@tanstack/react-query'

import { Button, ErrorNotice, Field, Input, Notice } from '@/components/ui'
import { transferApi } from '@/ipc/transfer'
import s from './Transfer.module.css'

interface Props {
  studentId: number
  studentName: string
  classLabel: string
  classNo: number | null
  schoolYear: number
  onCancel: () => void
  onDone: () => void
}

function todayIso(): string {
  const d = new Date()
  const p = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`
}

/**
 * 전출 처리 확인.
 *
 * 전출은 학생을 **현재 명단에서 빼는** 일이라 되돌리기 전까지 눈에 보이지 않는다.
 * 그래서 바로 처리하지 않고 지금 학적과 날짜를 보여 준 뒤 한 번 더 묻는다.
 */
export function TransferOutDialog({
  studentId,
  studentName,
  classLabel,
  classNo,
  schoolYear,
  onCancel,
  onDone,
}: Props) {
  const [date, setDate] = useState(todayIso())
  const [toSchool, setToSchool] = useState('')
  const [note, setNote] = useState('')

  const run = useMutation({
    mutationFn: () =>
      transferApi.transferOut({
        studentId,
        schoolYear,
        date,
        toSchool: toSchool.trim() || null,
        note: note.trim() || null,
      }),
    onSuccess: onDone,
  })

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && !run.isPending) onCancel()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [run.isPending, onCancel])

  return (
    <div className={s.overlay} onClick={() => !run.isPending && onCancel()}>
      <div
        className={s.box}
        role="dialog"
        aria-modal="true"
        aria-label="전출 처리"
        onClick={(e) => e.stopPropagation()}
      >
        <header className={s.boxHead}>
          <h2 className={s.boxTitle}>{studentName} 학생을 전출 처리하시겠습니까?</h2>
        </header>

        <div className={s.boxBody}>
          <ErrorNotice error={run.error} />

          <div className={s.kv}>
            <span className={s.kvKey}>현재 학적</span>
            <span className={s.kvVal}>
              {classLabel}
              {classNo != null ? ` ${classNo}번` : ''}
            </span>
          </div>

          <Field
            label="전출일"
            hint="앞으로 올 날도 됩니다 — 그날까지는 명단에 그대로 있고 [전출 예정]으로 보입니다"
          >
            <Input type="date" value={date} onChange={(e) => setDate(e.target.value)} />
          </Field>
          <Field label="전출 학교·지역" hint="아는 만큼만 적어 주세요">
            <Input
              value={toSchool}
              onChange={(e) => setToSchool(e.target.value)}
              placeholder="예: ○○초등학교"
            />
          </Field>
          <Field label="비고">
            <Input value={note} onChange={(e) => setNote(e.target.value)} />
          </Field>

          <Notice tone="warn">
            전출 처리 후 현재 학생명단에서는 제외되며 <b>전출생</b> 화면에서 확인할 수
            있습니다. 학생 자료는 지워지지 않고, 잘못 눌렀다면 전출생 화면에서 되돌릴 수
            있습니다.
          </Notice>
        </div>

        <footer className={s.boxFoot}>
          <span className={s.boxSpacer} />
          <Button variant="outline" onClick={onCancel} disabled={run.isPending}>
            취소
          </Button>
          <Button
            variant="danger"
            onClick={() => run.mutate()}
            disabled={run.isPending || !date.trim()}
          >
            {run.isPending ? '처리 중…' : '전출 처리'}
          </Button>
        </footer>
      </div>
    </div>
  )
}
