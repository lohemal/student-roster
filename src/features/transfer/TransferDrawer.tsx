import { useEffect, useMemo, useState } from 'react'
import { useMutation, useQuery } from '@tanstack/react-query'
import { X } from 'lucide-react'

import {
  Button,
  Drawer,
  DrawerSpacer,
  ErrorNotice,
  Field,
  Input,
  Notice,
} from '@/components/ui'
import { StudentForm, emptyStudent } from '@/features/student/StudentForm'
import type { StudentInput } from '@/ipc/student'
import { transferApi, type StudentMatch } from '@/ipc/transfer'
import { ClassCountsTable } from './ClassCountsTable'
import { StudentLookup } from './StudentLookup'
import s from './Transfer.module.css'

/** `in` = 전입 등록, `pastOut` = 지난 전출생 직접 넣기 */
export type TransferMode = 'in' | 'pastOut'

interface Props {
  mode: TransferMode
  schoolYear: number
  classSuggestions: string[]
  onClose: () => void
  onDone: (message: string) => void
}

const TEXT = {
  in: {
    title: '전입생 등록',
    dateLabel: '전입일',
    dateHint: '실제로 학교에 온 날입니다',
    submit: '전입 등록',
  },
  pastOut: {
    title: '지난 전출생 추가',
    dateLabel: '전출일',
    dateHint: '이미 학교를 떠난 날입니다',
    submit: '전출생으로 등록',
  },
}

/** 오늘을 `YYYY-MM-DD` 로 */
function todayIso(): string {
  const d = new Date()
  const p = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`
}

/**
 * 전입 등록 / 지난 전출생 추가.
 *
 * 두 화면은 하는 일이 다르지만 **기존 학생을 먼저 찾고, 같은 등록 폼을 쓰고, 날짜를
 * 받는다**는 흐름이 같다. 그래서 하나로 두고 문구와 저장하는 곳만 나눈다.
 */
export function TransferDrawer({
  mode,
  schoolYear,
  classSuggestions,
  onClose,
  onDone,
}: Props) {
  const t = TEXT[mode]
  const [step, setStep] = useState<'lookup' | 'form'>('lookup')
  const [picked, setPicked] = useState<StudentMatch | null>(null)
  const [form, setForm] = useState<StudentInput>(() => emptyStudent(schoolYear))
  const [date, setDate] = useState(todayIso())
  const [toSchool, setToSchool] = useState('')
  const [note, setNote] = useState('')

  const counts = useQuery({
    queryKey: ['class-counts', schoolYear, form.grade],
    queryFn: () => transferApi.classCounts(schoolYear, form.grade),
    enabled: step === 'form',
  })

  const used = useQuery({
    queryKey: ['used-numbers', schoolYear, form.grade, form.className],
    queryFn: () =>
      transferApi.usedNumbers(schoolYear, form.grade, form.className.trim() || null),
    enabled: step === 'form',
  })

  // 반을 바꾸면 번호 겹침 안내도 새로 본다
  useEffect(() => {
    if (step === 'form') used.refetch()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [form.grade, form.className])

  const numberTaken = useMemo(
    () => form.classNo != null && (used.data ?? []).includes(form.classNo),
    [form.classNo, used.data],
  )

  const save = useMutation({
    mutationFn: async () => {
      const student = { ...form, schoolYear }
      if (mode === 'in') {
        const r = await transferApi.transferIn({
          studentId: picked?.studentId ?? null,
          student,
          date,
        })
        const bits = [`${r.studentLabel} 학생을 전입 등록했습니다.`]
        if (r.returned) bits.push('같은 학년도 학적을 되살렸습니다.')
        if (r.siblingCandidates > 0)
          bits.push(`본교 형제 후보 ${r.siblingCandidates}명이 발견되었습니다.`)
        if (r.numberDup) bits.push('같은 반에 번호가 겹칩니다. 확인 필요에서 정리해 주세요.')
        return bits.join(' ')
      }
      await transferApi.pastOut({
        studentId: picked?.studentId ?? null,
        student,
        date,
        toSchool: toSchool.trim() || null,
        note: note.trim() || null,
      })
      return `${form.name} 학생을 전출생으로 등록했습니다.`
    },
    onSuccess: (msg) => onDone(msg),
  })

  const startNew = (name: string, birth: string) => {
    setPicked(null)
    setForm({ ...emptyStudent(schoolYear), name, birthRaw: birth })
    setStep('form')
  }

  const startExisting = (m: StudentMatch) => {
    setPicked(m)
    // 지난 학적을 시작값으로 삼는다 — 대개 학년만 올라간다
    const last = m.history[0]
    setForm({
      ...emptyStudent(schoolYear),
      name: m.name,
      gender: m.gender ?? '',
      birthRaw: m.birthRaw ?? m.birthDate ?? '',
      grade: last ? Math.min(6, last.schoolYear === schoolYear ? gradeOf(last.whereAt) : gradeOf(last.whereAt) + 1) : 1,
    })
    setStep('form')
  }

  return (
    <Drawer
      title={t.title}
      onClose={onClose}
      footer={
        step === 'form' ? (
          <>
            <Button variant="ghost" onClick={() => setStep('lookup')}>
              다시 찾기
            </Button>
            <DrawerSpacer />
            <Button variant="outline" onClick={onClose}>
              취소
            </Button>
            <Button
              variant="primary"
              onClick={() => save.mutate()}
              disabled={save.isPending || !form.name.trim() || !date.trim()}
            >
              {t.submit}
            </Button>
          </>
        ) : (
          <>
            <DrawerSpacer />
            <Button variant="outline" onClick={onClose}>
              닫기
            </Button>
          </>
        )
      }
    >
      {step === 'lookup' ? (
        <StudentLookup schoolYear={schoolYear} onPick={startExisting} onNew={startNew} />
      ) : (
        <div className={s.formWrap}>
          <ErrorNotice error={save.error} />

          {picked ? (
            <div className={s.pickedStudent}>
              <span className={s.pickedName}>{picked.name}</span>
              <span>이미 등록된 학생입니다. 같은 학생으로 이어서 기록합니다.</span>
              <span className={s.pickedSpacer} />
              <Button size="sm" variant="ghost" icon={X} onClick={() => setStep('lookup')}>
                고르기 취소
              </Button>
            </div>
          ) : (
            <Notice tone="info">처음 오는 학생으로 새로 등록합니다.</Notice>
          )}

          <div>
            <div className={s.sectionTitle}>{t.dateLabel}</div>
            <div className={s.moveFields}>
              <Field label={t.dateLabel} hint={t.dateHint}>
                <Input type="date" value={date} onChange={(e) => setDate(e.target.value)} />
              </Field>
              {mode === 'pastOut' && (
                <Field label="전출 학교·지역" hint="아는 만큼만 적어 주세요">
                  <Input
                    value={toSchool}
                    onChange={(e) => setToSchool(e.target.value)}
                    placeholder="예: ○○초등학교"
                  />
                </Field>
              )}
            </div>
            {mode === 'pastOut' && (
              <Field label="비고">
                <Input value={note} onChange={(e) => setNote(e.target.value)} />
              </Field>
            )}
          </div>

          <StudentForm value={form} onChange={setForm} classSuggestions={classSuggestions} />

          {numberTaken && (
            <Notice tone="warn">
              현재 {form.grade}-{form.className || '미정'}반에서 {form.classNo}번을 쓰는 학생이
              있습니다. 그대로 저장해도 되지만 <b>번호 중복</b>으로 표시됩니다.
            </Notice>
          )}

          {mode === 'in' && (
            <ClassCountsTable
              counts={counts.data}
              picked={form.className}
              gender={form.gender as 'M' | 'F' | ''}
            />
          )}
        </div>
      )}
    </Drawer>
  )
}

/** `2-나리 7번` 에서 학년만 뽑는다 */
function gradeOf(whereAt: string): number {
  const n = Number(whereAt.split('-')[0])
  return Number.isFinite(n) && n >= 1 && n <= 6 ? n : 1
}
