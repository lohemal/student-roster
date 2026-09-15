import { useEffect, useState } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { GraduationCap, RotateCcw, Search } from 'lucide-react'

import {
  Button,
  Empty,
  ErrorNotice,
  Input,
  Notice,
  Page,
  Select,
  TableWrap,
  tableClass,
} from '@/components/ui'
import { StudentDrawer } from '@/features/student/StudentDrawer'
import { graduationApi, type GradRow } from '@/ipc/graduation'
import { studentApi } from '@/ipc/student'
import { birthCell, genderLabel } from '@/lib/format'
import { yearLabel } from '@/lib/schoolYear'
import s from './GraduatesPage.module.css'

const n = (v: number) => v.toLocaleString('ko-KR')

/**
 * 졸업생.
 *
 * 졸업생을 따로 복사해 두지 않는다 — 같은 학생 자료를 졸업 학년도 기준으로 볼 뿐이다.
 * 그래서 학생 상세 창도 그대로 쓴다. 학년·반·번호는 **졸업 당시 학적**이고,
 * 전환이 지난 학년도를 고치지 않으므로 나중에 바뀌지 않는다.
 */
export function GraduatesPage() {
  const qc = useQueryClient()
  const [year, setYear] = useState<number | null>(null)
  const [q, setQ] = useState('')
  const [openStudent, setOpenStudent] = useState<number | undefined>(undefined)
  const [confirm, setConfirm] = useState<GradRow | null>(null)
  const [error, setError] = useState<unknown>(null)

  const years = useQuery({ queryKey: ['graduation-years'], queryFn: graduationApi.years })
  const picked = year ?? years.data?.[0]?.schoolYear ?? null

  const list = useQuery({
    queryKey: ['graduates', picked, q],
    queryFn: () => graduationApi.list(picked!, q.trim() || undefined),
    enabled: picked != null,
    placeholderData: (prev) => prev,
  })

  const classOptions = useQuery({
    queryKey: ['class-options', picked],
    queryFn: () => studentApi.classOptions(picked!),
    enabled: picked != null,
  })

  useEffect(() => setConfirm(null), [picked, q])

  const cancel = async (row: GradRow) => {
    setError(null)
    try {
      await graduationApi.cancel(row.studentId)
      setConfirm(null)
      qc.invalidateQueries()
    } catch (e) {
      setError(e)
    }
  }

  if (years.data && years.data.length === 0) {
    return (
      <Page title="졸업생">
        <Empty
          icon={GraduationCap}
          title="아직 졸업생이 없습니다"
          description="학년도 전환에서 6학년을 졸업 처리하면 여기에 남습니다."
        />
      </Page>
    )
  }

  const rows = list.data ?? []

  return (
    <Page
      title="졸업생"
      year={picked != null ? yearLabel(picked) : undefined}
      description="졸업 당시 학년·반·번호를 그대로 보여 줍니다"
    >
      <ErrorNotice error={error ?? list.error ?? years.error} />

      <div className={s.filters}>
        <Select
          className={s.compact}
          value={picked ?? ''}
          onChange={(e) => setYear(Number(e.target.value))}
          aria-label="졸업 학년도"
        >
          {(years.data ?? []).map((y) => (
            <option key={y.schoolYear} value={y.schoolYear}>
              {y.schoolYear}학년도 졸업 ({n(y.count)}명)
            </option>
          ))}
        </Select>

        <div className={s.search}>
          <Search size={15} className={s.searchIcon} />
          <Input
            placeholder="이름으로 찾기"
            value={q}
            onChange={(e) => setQ(e.target.value)}
          />
        </div>

        <span className={s.count}>{n(rows.length)}명</span>
      </div>

      {confirm && (
        <Notice tone="warn">
          <strong>{confirm.name}</strong> 학생의 졸업 처리를 되돌립니다. {confirm.schoolYear}학년도
          명단으로 돌아오고, 처리한 기록은 학적 이력에 남습니다. 다음 학년도 학적은 만들어지지
          않으므로 필요하면 직접 등록해 주세요.
          <div className={s.confirmRow}>
            <Button variant="danger" onClick={() => cancel(confirm)}>
              졸업 취소
            </Button>
            <Button variant="ghost" onClick={() => setConfirm(null)}>
              그만두기
            </Button>
          </div>
        </Notice>
      )}

      {rows.length === 0 ? (
        <Empty
          icon={GraduationCap}
          title={q ? '찾는 졸업생이 없습니다' : '이 학년도에는 졸업생이 없습니다'}
        />
      ) : (
        <TableWrap>
          <thead>
            <tr>
              <th>학년</th>
              <th>반</th>
              <th className={tableClass.num}>번호</th>
              <th>이름</th>
              <th>성별</th>
              <th>생년월일</th>
              <th>졸업일</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => (
              <tr key={r.studentId}>
                <td onClick={() => setOpenStudent(r.studentId)}>
                  {r.grade != null ? `${r.grade}학년` : '—'}
                </td>
                <td onClick={() => setOpenStudent(r.studentId)}>{r.className ?? '—'}</td>
                <td className={tableClass.num} onClick={() => setOpenStudent(r.studentId)}>
                  {r.classNo ?? ''}
                </td>
                <td className={s.name} onClick={() => setOpenStudent(r.studentId)}>
                  {r.name}
                </td>
                <td onClick={() => setOpenStudent(r.studentId)}>{genderLabel(r.gender as 'M' | 'F' | null)}</td>
                <td onClick={() => setOpenStudent(r.studentId)}>
                  {birthCell(r.birthDate, r.birthRaw)}
                </td>
                <td className={tableClass.muted}>{r.graduatedAt ?? '—'}</td>
                <td>
                  <Button
                    size="sm"
                    variant="ghost"
                    icon={RotateCcw}
                    onClick={() => setConfirm(r)}
                  >
                    졸업 취소
                  </Button>
                </td>
              </tr>
            ))}
          </tbody>
        </TableWrap>
      )}

      {openStudent !== undefined && picked != null && (
        <StudentDrawer
          studentId={openStudent}
          schoolYear={picked}
          classSuggestions={Array.from(new Set((classOptions.data ?? []).map((c) => c.className)))}
          onClose={() => setOpenStudent(undefined)}
          onSaved={() => qc.invalidateQueries({ queryKey: ['graduates'] })}
        />
      )}
    </Page>
  )
}
