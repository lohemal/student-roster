import { useState } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { LogIn, Search, UserPlus } from 'lucide-react'

import {
  Badge,
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
import {
  CancelInButton,
  PendingBadge,
  RescheduleButton,
} from '@/features/transfer/PendingMove'
import { TransferDrawer } from '@/features/transfer/TransferDrawer'
import { settingsApi } from '@/ipc/settings'
import { studentApi } from '@/ipc/student'
import { transferApi } from '@/ipc/transfer'
import { yearLabel } from '@/lib/schoolYear'
import { dotDate, genderLabel } from '@/lib/format'
import s from '@/features/transfer/Transfer.module.css'

/**
 * 전입생 화면.
 *
 * 올해 들어온 학생을 모아 보고 새 전입을 처리한다. 전입생은 **현재 재학생**이므로
 * 학생명단에도 함께 나온다. 여기는 '언제 누가 들어왔는지' 를 보는 자리다.
 */
export function TransferInPage() {
  const qc = useQueryClient()
  const settings = useQuery({ queryKey: ['settings'], queryFn: settingsApi.get })
  const schoolYear = settings.data?.currentYear ?? null

  const [grade, setGrade] = useState<number | null>(null)
  const [q, setQ] = useState('')
  const [open, setOpen] = useState(false)
  const [openStudent, setOpenStudent] = useState<number | null>(null)
  const [done, setDone] = useState<string | null>(null)

  const classOptions = useQuery({
    queryKey: ['class-options', schoolYear],
    queryFn: () => studentApi.classOptions(schoolYear!),
    enabled: schoolYear != null,
  })

  const list = useQuery({
    queryKey: ['transfer', 'in', schoolYear, grade, q],
    queryFn: () => transferApi.list(schoolYear!, true, grade, q.trim() || null),
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

  if (schoolYear == null) {
    return (
      <Page title="전입생">
        <Empty
          icon={LogIn}
          title="먼저 학년도를 만들어 주세요"
          description="설정 화면에서 현재 학년도를 지정하면 전입을 처리할 수 있습니다."
        />
      </Page>
    )
  }

  const rows = list.data ?? []

  return (
    <Page
      title="전입생"
      year={yearLabel(schoolYear)}
      actions={
        <Button variant="primary" icon={UserPlus} onClick={() => setOpen(true)}>
          전입생 등록
        </Button>
      }
    >
      <ErrorNotice error={list.error} />

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
          icon={LogIn}
          title="올해 전입한 학생이 없습니다"
          description="학기 중에 새로 온 학생이 있으면 오른쪽 위 [전입생 등록]으로 넣어 주세요."
        />
      ) : (
        <TableWrap>
          <thead>
            <tr>
              <th style={{ width: 104 }}>전입일</th>
              <th style={{ width: 62 }}>학년</th>
              <th style={{ width: 88 }}>반</th>
              <th style={{ width: 56 }} className={tableClass.num}>
                번호
              </th>
              <th style={{ width: 120 }}>이름</th>
              <th style={{ width: 48 }} className={tableClass.center}>
                성별
              </th>
              <th style={{ width: 150 }}>상태</th>
              <th style={{ width: 210 }} />
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
                <td className={s.badgeCell}>
                  {r.pending ? (
                    <PendingBadge row={r} />
                  ) : (
                    <Badge tone="info">{r.statusLabel}</Badge>
                  )}
                  {r.issueCount > 0 && <Badge tone="warn">확인 {r.issueCount}</Badge>}
                </td>
                <td className={s.badgeCell}>
                  {r.pending === 'IN' && (
                    <>
                      <RescheduleButton row={r} schoolYear={schoolYear} onDone={refresh} />
                      <CancelInButton
                        row={r}
                        schoolYear={schoolYear}
                        onDone={(msg) => {
                          setDone(msg)
                          refresh()
                        }}
                      />
                    </>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </TableWrap>
      )}

      <div className={s.note}>
        <Notice tone="info">
          전입생은 <b>현재 재학생</b>이므로 학생명단에도 함께 나옵니다. 학교를 떠난 학생은
          <b> 전출생</b> 화면에서 봅니다. <b>전입 예정</b>은 전입일이 되어야 학생명단·통계·
          명단 파일에 들어갑니다 — 그날이 되면 저절로 바뀌므로 따로 누를 것이 없습니다.
        </Notice>
      </div>

      {open && (
        <TransferDrawer
          mode="in"
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
