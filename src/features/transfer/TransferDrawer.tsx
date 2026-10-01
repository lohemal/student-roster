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
import { studentApi, type StudentDetail, type StudentInput } from '@/ipc/student'
import { transferApi, type StudentMatch } from '@/ipc/transfer'
import { birthDisplay } from '@/lib/format'
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
    dateHint: '앞으로 올 날도 됩니다 — 그날까지는 전입 예정입니다',
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
  /** 고른 기존 학생의 전체 자료 — 무엇을 이어 쓰는지 사람이 볼 수 있게 한다 */
  const [detail, setDetail] = useState<StudentDetail | null>(null)
  const [loadError, setLoadError] = useState<unknown>(null)
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
    setDetail(null)
    setLoadError(null)
    setForm({ ...emptyStudent(schoolYear), name, birthRaw: birth })
    setStep('form')
  }

  /**
   * 기존 학생을 골랐다.
   *
   * **이미 DB 에 있는 학생정보를 다시 입력하게 하지 않는다.** 주소·보호자·연락처·비고를
   * 그대로 싣고, 사람이 새로 정할 것은 이동 정보(날짜)와 새 학적(학년·반·번호)뿐이다.
   * 학생 자료를 복사하는 것이 아니라 **같은 `studentId` 를 그대로 쓴다.**
   */
  const startExisting = async (m: StudentMatch) => {
    setPicked(m)
    setLoadError(null)
    // 지난 학적을 시작값으로 삼는다 — 대개 학년만 올라간다
    const last = m.history[0]
    const grade = last
      ? Math.min(
          6,
          last.schoolYear === schoolYear
            ? gradeOf(last.whereAt)
            : gradeOf(last.whereAt) + 1,
        )
      : 1
    const base: StudentInput = {
      ...emptyStudent(schoolYear),
      name: m.name,
      gender: m.gender,
      birthRaw: m.birthRaw ?? m.birthDate ?? '',
      grade,
    }
    setForm(base)
    setStep('form')

    try {
      const d = await studentApi.get(m.studentId, schoolYear)
      setDetail(d)
      setForm({
        ...base,
        name: d.name,
        gender: d.gender,
        birthRaw: d.birthRaw ?? d.birthDate ?? '',
        addressRaw: d.addressRaw ?? '',
        fatherName: d.fatherName ?? '',
        motherName: d.motherName ?? '',
        fatherPhone: d.fatherPhone ?? '',
        motherPhone: d.motherPhone ?? '',
        primaryPhone: d.primaryPhone ?? '',
        note: d.note ?? '',
        // 올해 학적이 이미 있으면(되돌아오는 경우) 그 자리를 그대로 보여 준다
        className: d.enrollment?.className ?? '',
        classNo: d.enrollment?.classNo ?? null,
        grade: d.enrollment?.grade ?? grade,
        // 직접 지정한 주소 분류는 다시 판정하지 않는다
        keepManualAddress: d.addressSource === 'MANUAL',
      })
    } catch (e) {
      // 불러오지 못해도 등록은 막지 않는다 — 무엇이 비었는지만 알린다
      setLoadError(e)
    }
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
            <PickedStudent
              match={picked}
              detail={detail}
              error={loadError}
              onUndo={() => setStep('lookup')}
            />
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

/**
 * 고른 기존 학생이 무엇을 이어 쓰는지.
 *
 * **이미 들어 있는 것을 보여 주어 다시 입력하지 않게 한다.** 접었다 펼 수 있게 두어
 * 평소에는 한 줄만 차지한다.
 */
function PickedStudent({
  match,
  detail,
  error,
  onUndo,
}: {
  match: StudentMatch
  detail: StudentDetail | null
  error: unknown
  onUndo: () => void
}) {
  const [open, setOpen] = useState(false)
  const last = match.history[0]
  const guardians = detail
    ? [
        detail.fatherName && `부 ${detail.fatherName}`,
        detail.motherName && `모 ${detail.motherName}`,
        detail.fatherPhone && `부 연락처 ${detail.fatherPhone}`,
        detail.motherPhone && `모 연락처 ${detail.motherPhone}`,
        detail.primaryPhone && `주보호자 ${detail.primaryPhone}`,
      ].filter(Boolean)
    : []

  return (
    <div className={s.picked}>
      <div className={s.pickedStudent}>
        <span className={s.pickedName}>{match.name}</span>
        <span>
          기존 학생 정보를 불러왔습니다. 같은 학생으로 이어서 기록합니다
          {last && <> · 이전 학적 {last.schoolYear}학년도 {last.whereAt}</>}
        </span>
        <span className={s.pickedSpacer} />
        <Button size="sm" variant="ghost" onClick={() => setOpen((v) => !v)}>
          {open ? '접기' : '자세히'}
        </Button>
        <Button size="sm" variant="ghost" icon={X} onClick={onUndo}>
          고르기 취소
        </Button>
      </div>

      <ErrorNotice error={error} />

      {open && (
        <dl className={s.pickedDetail}>
          <dt>생년월일</dt>
          <dd>{birthDisplay(detail?.birthDate ?? match.birthDate) || match.birthRaw || '없음'}</dd>
          <dt>주소</dt>
          <dd>
            {detail?.addressRaw || <span className={s.pickedFaint}>없음</span>}
            {detail?.addressCategory && <> · {detail.addressCategory}</>}
          </dd>
          <dt>보호자</dt>
          <dd>
            {guardians.length > 0 ? (
              guardians.join(' · ')
            ) : (
              <span className={s.pickedFaint}>등록된 정보 없음</span>
            )}
          </dd>
          <dt>비고</dt>
          <dd>{detail?.note || <span className={s.pickedFaint}>없음</span>}</dd>
          <dt>학적 이력</dt>
          <dd>
            {match.history.length === 0
              ? '없음'
              : match.history
                  .map((h) => `${h.schoolYear}학년도 ${h.whereAt} (${h.statusLabel})`)
                  .join(' · ')}
          </dd>
        </dl>
      )}
    </div>
  )
}
