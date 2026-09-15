import { useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { LogOut, RotateCcw, Search, UserPlus } from 'lucide-react'

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
import { TransferDrawer } from '@/features/transfer/TransferDrawer'
import { settingsApi } from '@/ipc/settings'
import { studentApi } from '@/ipc/student'
import { transferApi, type MoveRow } from '@/ipc/transfer'
import { dotDate, genderLabel } from '@/lib/format'
import { yearLabel } from '@/lib/schoolYear'
import s from '@/features/transfer/Transfer.module.css'

/**
 * 전출생 화면.
 *
 * 올해 학교를 떠난 학생을 본다. **학생 자료는 지워지지 않는다** — 현재 명단에서만
 * 빠져 있을 뿐이다. 잘못 처리했으면 여기서 되돌린다.
 */
export function TransferOutPage() {
  const qc = useQueryClient()
  const settings = useQuery({ queryKey: ['settings'], queryFn: settingsApi.get })
  const schoolYear = settings.data?.currentYear ?? null

  const [grade, setGrade] = useState<number | null>(null)
  const [q, setQ] = useState('')
  const [open, setOpen] = useState(false)
  const [openStudent, setOpenStudent] = useState<number | null>(null)
  const [done, setDone] = useState<string | null>(null)
  const [confirmCancel, setConfirmCancel] = useState<MoveRow | null>(null)

  const classOptions = useQuery({
    queryKey: ['class-options', schoolYear],
    queryFn: () => studentApi.classOptions(schoolYear!),
    enabled: schoolYear != null,
  })

  const list = useQuery({
    queryKey: ['transfer', 'out', schoolYear, grade, q],
    queryFn: () => transferApi.list(schoolYear!, false, grade, q.trim() || null),
    enabled: schoolYear != null,
  })

  const refresh = () => {
    qc.invalidateQueries({ queryKey: ['transfer'] })
    qc.invalidateQueries({ queryKey: ['students'] })
    qc.invalidateQueries({ queryKey: ['class-options'] })
    qc.invalidateQueries({ queryKey: ['class-counts'] })
    qc.invalidateQueries({ queryKey: ['used-numbers'] })
    qc.invalidateQueries({ queryKey: ['issue-summary'] })
    qc.invalidateQueries({ queryKey: ['issues'] })
    qc.invalidateQueries({ queryKey: ['siblings'] })
    qc.invalidateQueries({ queryKey: ['settings'] })
  }

  const cancel = useMutation({
    mutationFn: (row: MoveRow) => transferApi.cancelOut(row.studentId, schoolYear!),
    onSuccess: (_, row) => {
      setConfirmCancel(null)
      setDone(`${row.name} 학생의 전출 처리를 되돌렸습니다. 학생명단으로 돌아왔습니다.`)
      refresh()
    },
  })

  if (schoolYear == null) {
    return (
      <Page title="전출생">
        <Empty
          icon={LogOut}
          title="먼저 학년도를 만들어 주세요"
          description="설정 화면에서 현재 학년도를 지정하면 전출을 관리할 수 있습니다."
        />
      </Page>
    )
  }

  const rows = list.data ?? []

  return (
    <Page
      title="전출생"
      year={yearLabel(schoolYear)}
      actions={
        <Button variant="outline" icon={UserPlus} onClick={() => setOpen(true)}>
          지난 전출생 추가
        </Button>
      }
    >
      <ErrorNotice error={list.error ?? cancel.error} />

      {done && (
        <div style={{ marginBottom: 12 }}>
          <Notice tone="success">{done}</Notice>
        </div>
      )}

      <div className={s.filters}>
        <div className={s.search}>
          <Search size={16} className={s.searchIcon} />
          <Input
            className={s.searchInput}
            value={q}
            onChange={(e) => setQ(e.target.value)}
            placeholder="이름으로 찾기"
          />
        </div>
        <Select
          className={s.compact}
          value={grade ?? ''}
          onChange={(e) => setGrade(e.target.value === '' ? null : Number(e.target.value))}
        >
          <option value="">전체 학년</option>
          {[1, 2, 3, 4, 5, 6].map((g) => (
            <option key={g} value={g}>
              {g}학년
            </option>
          ))}
        </Select>
      </div>

      <div className={s.count}>{rows.length.toLocaleString('ko-KR')}명</div>

      {rows.length === 0 ? (
        <Empty
          icon={LogOut}
          title="올해 전출한 학생이 없습니다"
          description="학생명단에서 학생을 열어 [전출 처리]를 하면 여기에 모입니다. 프로그램을 쓰기 전에 이미 떠난 학생은 [지난 전출생 추가]로 넣을 수 있습니다."
        />
      ) : (
        <TableWrap>
          <thead>
            <tr>
              <th style={{ width: 104 }}>전출일</th>
              <th style={{ width: 62 }}>학년</th>
              <th style={{ width: 88 }}>반</th>
              <th style={{ width: 56 }} className={tableClass.num}>
                번호
              </th>
              <th style={{ width: 120 }}>이름</th>
              <th style={{ width: 48 }} className={tableClass.center}>
                성별
              </th>
              <th style={{ width: 150 }}>전출 학교·지역</th>
              <th>비고</th>
              <th style={{ width: 128 }} />
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => (
              <tr
                key={r.studentId}
                className={openStudent === r.studentId ? tableClass.rowActive : undefined}
                onClick={() => setOpenStudent(r.studentId)}
              >
                <td className={s.dateCell}>{dotDate(r.date)}</td>
                <td>{r.grade}학년</td>
                <td>{r.className ?? <span className={tableClass.muted}>미정</span>}</td>
                <td className={tableClass.num}>{r.classNo ?? ''}</td>
                <td style={{ fontWeight: 600 }}>{r.name}</td>
                <td className={tableClass.center}>{genderLabel(r.gender)}</td>
                <td>{r.toSchool ?? <span className={tableClass.muted}>모름</span>}</td>
                <td className={s.noteCell} title={r.note ?? ''}>
                  {r.note ?? ''}
                </td>
                <td>
                  <Button
                    size="sm"
                    variant="ghost"
                    icon={RotateCcw}
                    onClick={(e) => {
                      e.stopPropagation()
                      setConfirmCancel(r)
                    }}
                  >
                    전출 취소
                  </Button>
                </td>
              </tr>
            ))}
          </tbody>
        </TableWrap>
      )}

      <div className={s.note}>
        <Notice tone="info">
          전출한 학생의 자료는 <b>지워지지 않습니다.</b> 현재 학생명단과 인원 집계에서만
          빠져 있습니다. 잘못 처리했다면 [전출 취소]로 되돌릴 수 있고, 그 기록도 학적
          이력에 남습니다.
        </Notice>
      </div>

      {confirmCancel && (
        <div className={s.overlay} onClick={() => !cancel.isPending && setConfirmCancel(null)}>
          <div className={s.box} role="dialog" aria-modal="true" onClick={(e) => e.stopPropagation()}>
            <header className={s.boxHead}>
              <h2 className={s.boxTitle}>
                {confirmCancel.name} 학생의 전출을 되돌리시겠습니까?
              </h2>
            </header>
            <div className={s.boxBody}>
              <div className={s.kv}>
                <span className={s.kvKey}>전출일</span>
                <span className={s.kvVal}>{dotDate(confirmCancel.date) || '기록 없음'}</span>
                <span className={s.kvKey}>학적</span>
                <span className={s.kvVal}>
                  {confirmCancel.classLabel}
                  {confirmCancel.classNo != null ? ` ${confirmCancel.classNo}번` : ''}
                </span>
              </div>
              <Notice tone="info">
                현재 학생명단으로 돌아오고 반별 인원에도 다시 들어갑니다. 전출했던 기록은
                학적 이력에 그대로 남고, 되돌린 사실도 함께 적힙니다.
              </Notice>
            </div>
            <footer className={s.boxFoot}>
              <span className={s.boxSpacer} />
              <Button
                variant="outline"
                onClick={() => setConfirmCancel(null)}
                disabled={cancel.isPending}
              >
                취소
              </Button>
              <Button
                variant="primary"
                onClick={() => cancel.mutate(confirmCancel)}
                disabled={cancel.isPending}
              >
                {cancel.isPending ? '되돌리는 중…' : '전출 취소'}
              </Button>
            </footer>
          </div>
        </div>
      )}

      {open && (
        <TransferDrawer
          mode="pastOut"
          schoolYear={schoolYear}
          classSuggestions={Array.from(
            new Set((classOptions.data ?? []).map((c) => c.className)),
          )}
          onClose={() => setOpen(false)}
          onDone={(msg) => {
            setOpen(false)
            setDone(msg)
            refresh()
          }}
        />
      )}

      {openStudent != null && (
        <StudentDrawer
          studentId={openStudent}
          schoolYear={schoolYear}
          classSuggestions={Array.from(
            new Set((classOptions.data ?? []).map((c) => c.className)),
          )}
          onClose={() => setOpenStudent(null)}
          onSaved={refresh}
        />
      )}
    </Page>
  )
}
