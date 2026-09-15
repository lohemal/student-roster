import { useEffect, useMemo, useState } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import {
  ChevronLeft,
  ChevronRight,
  ListFilter as FilterIcon,
  Search,
  SlidersHorizontal,
  UserPlus,
  Users,
  X,
} from 'lucide-react'

import {
  Badge,
  Button,
  Empty,
  ErrorNotice,
  Field,
  Input,
  Select,
  TableWrap,
  tableClass,
  Page,
} from '@/components/ui'
import { StudentDrawer } from '@/features/student/StudentDrawer'
import { issueApi } from '@/ipc/issue'
import { settingsApi } from '@/ipc/settings'
import { studentApi, type ListFilter, type StatusFilter } from '@/ipc/student'
import { birthCell, genderLabel, issueBadge, rangeLabel, statusBadge } from '@/lib/format'
import { yearLabel } from '@/lib/schoolYear'
import s from './StudentsPage.module.css'

const PAGE_SIZE = 100

const STATUS_OPTIONS: { value: StatusFilter; label: string }[] = [
  { value: 'ACTIVE', label: '재학생' },
  { value: 'ALL', label: '전체' },
  { value: 'TRANSFER_IN', label: '전입' },
  { value: 'TRANSFER_OUT', label: '전출' },
]

/** 항목별 상세 검색 칸 */
interface Advanced {
  name: string
  classNo: string
  address: string
  fatherPhone: string
  motherPhone: string
  primaryPhone: string
}

const EMPTY_ADVANCED: Advanced = {
  name: '',
  classNo: '',
  address: '',
  fatherPhone: '',
  motherPhone: '',
  primaryPhone: '',
}

export function StudentsPage() {
  const qc = useQueryClient()
  const settings = useQuery({ queryKey: ['settings'], queryFn: settingsApi.get })
  const schoolYear = settings.data?.currentYear ?? null

  const [q, setQ] = useState('')
  const [grade, setGrade] = useState<number | null>(null)
  const [className, setClassName] = useState<string | null>(null)
  const [status, setStatus] = useState<StatusFilter>('ACTIVE')
  const [onlyIssues, setOnlyIssues] = useState(false)
  const [showAdvanced, setShowAdvanced] = useState(false)
  const [adv, setAdv] = useState<Advanced>(EMPTY_ADVANCED)
  const [page, setPage] = useState(0)

  // 열려 있는 학생. undefined = 닫힘, null = 새 등록
  const [openStudent, setOpenStudent] = useState<number | null | undefined>(undefined)

  const classOptions = useQuery({
    queryKey: ['class-options', schoolYear],
    queryFn: () => studentApi.classOptions(schoolYear!),
    enabled: schoolYear != null,
  })

  const filter: ListFilter | null = useMemo(() => {
    if (schoolYear == null) return null
    return {
      schoolYear,
      q: q.trim() || undefined,
      grade,
      className,
      status,
      onlyIssues,
      name: adv.name.trim() || undefined,
      classNo: adv.classNo.trim() ? Number(adv.classNo) : null,
      address: adv.address.trim() || undefined,
      fatherPhone: adv.fatherPhone.trim() || undefined,
      motherPhone: adv.motherPhone.trim() || undefined,
      primaryPhone: adv.primaryPhone.trim() || undefined,
    }
  }, [schoolYear, q, grade, className, status, onlyIssues, adv])

  // 조건이 바뀌면 첫 쪽으로
  useEffect(() => {
    setPage(0)
  }, [q, grade, className, status, onlyIssues, adv])

  const list = useQuery({
    queryKey: ['students', filter, page],
    queryFn: () => studentApi.list(filter!, PAGE_SIZE, page * PAGE_SIZE),
    enabled: filter != null,
    placeholderData: (prev) => prev,
  })

  const issues = useQuery({
    queryKey: ['issue-summary', schoolYear],
    queryFn: () => issueApi.summary(schoolYear!),
    enabled: schoolYear != null,
  })

  const refresh = () => {
    qc.invalidateQueries({ queryKey: ['students'] })
    qc.invalidateQueries({ queryKey: ['class-options'] })
    qc.invalidateQueries({ queryKey: ['issue-summary'] })
    qc.invalidateQueries({ queryKey: ['settings'] })
  }

  const classesForGrade = (classOptions.data ?? []).filter(
    (c) => grade == null || c.grade === grade,
  )
  const suggestions = Array.from(new Set((classOptions.data ?? []).map((c) => c.className)))

  const rows = list.data?.rows ?? []
  const total = list.data?.total ?? 0
  const lastPage = Math.max(0, Math.ceil(total / PAGE_SIZE) - 1)
  const hasFilter =
    q.trim() !== '' ||
    grade != null ||
    className != null ||
    onlyIssues ||
    status !== 'ACTIVE' ||
    Object.values(adv).some((v) => v.trim() !== '')

  const resetFilters = () => {
    setQ('')
    setGrade(null)
    setClassName(null)
    setStatus('ACTIVE')
    setOnlyIssues(false)
    setAdv(EMPTY_ADVANCED)
  }

  if (schoolYear == null) {
    return (
      <Page title="학생명단">
        <Empty
          icon={Users}
          title="먼저 학년도를 만들어 주세요"
          description="설정 화면에서 현재 학년도를 지정하면 학생을 등록할 수 있습니다."
        />
      </Page>
    )
  }

  return (
    <Page
      title="학생명단"
      year={yearLabel(schoolYear)}
      actions={
        <Button variant="primary" icon={UserPlus} onClick={() => setOpenStudent(null)}>
          학생 등록
        </Button>
      }
    >
      <ErrorNotice error={list.error ?? classOptions.error} />

      <div className={s.filters}>
        <div className={s.filterRow}>
          <div className={s.search}>
            <Search size={16} className={s.searchIcon} />
            <Input
              className={s.searchInput}
              value={q}
              onChange={(e) => setQ(e.target.value)}
              placeholder="이름 · 연락처 · 주소 · 번호로 찾기 (1234 처럼 일부만 입력해도 됩니다)"
            />
            {q && (
              <button type="button" className={s.clearBtn} onClick={() => setQ('')} aria-label="지우기">
                <X size={15} />
              </button>
            )}
          </div>

          <Select
            className={s.compact}
            value={grade ?? ''}
            onChange={(e) => {
              setGrade(e.target.value === '' ? null : Number(e.target.value))
              setClassName(null)
            }}
          >
            <option value="">전체 학년</option>
            {[1, 2, 3, 4, 5, 6].map((g) => (
              <option key={g} value={g}>
                {g}학년
              </option>
            ))}
          </Select>

          <Select
            className={s.compact}
            value={className ?? ''}
            onChange={(e) => setClassName(e.target.value === '' ? null : e.target.value)}
          >
            <option value="">전체 반</option>
            {Array.from(new Set(classesForGrade.map((c) => c.className))).map((c) => (
              <option key={c} value={c}>
                {c}
              </option>
            ))}
          </Select>

          <Select
            className={s.compact}
            value={status}
            onChange={(e) => setStatus(e.target.value as StatusFilter)}
          >
            {STATUS_OPTIONS.map((o) => (
              <option key={o.value} value={o.value}>
                {o.label}
              </option>
            ))}
          </Select>

          <Button
            variant={showAdvanced ? 'primary' : 'outline'}
            icon={SlidersHorizontal}
            onClick={() => setShowAdvanced((v) => !v)}
          >
            상세 검색
          </Button>
        </div>

        {showAdvanced && (
          <div className={s.advanced}>
            <Field label="이름">
              <Input value={adv.name} onChange={(e) => setAdv({ ...adv, name: e.target.value })} />
            </Field>
            <Field label="번호">
              <Input
                type="number"
                min={1}
                value={adv.classNo}
                onChange={(e) => setAdv({ ...adv, classNo: e.target.value })}
              />
            </Field>
            <Field label="주소">
              <Input
                value={adv.address}
                onChange={(e) => setAdv({ ...adv, address: e.target.value })}
              />
            </Field>
            <Field label="부 연락처" hint="일부만 입력해도 됩니다">
              <Input
                value={adv.fatherPhone}
                onChange={(e) => setAdv({ ...adv, fatherPhone: e.target.value })}
              />
            </Field>
            <Field label="모 연락처" hint="일부만 입력해도 됩니다">
              <Input
                value={adv.motherPhone}
                onChange={(e) => setAdv({ ...adv, motherPhone: e.target.value })}
              />
            </Field>
            <Field label="주보호자 연락처" hint="일부만 입력해도 됩니다">
              <Input
                value={adv.primaryPhone}
                onChange={(e) => setAdv({ ...adv, primaryPhone: e.target.value })}
              />
            </Field>
          </div>
        )}

        <div className={s.summary}>
          <span className={s.summaryCount}>{rangeLabel(total, page * PAGE_SIZE, rows.length)}</span>
          {issues.data && issues.data.total > 0 && (
            <Button
              size="sm"
              variant={onlyIssues ? 'primary' : 'outline'}
              icon={FilterIcon}
              onClick={() => setOnlyIssues((v) => !v)}
            >
              확인 필요 {issues.data.total}건만 보기
            </Button>
          )}
          {hasFilter && (
            <Button size="sm" variant="ghost" onClick={resetFilters}>
              조건 지우기
            </Button>
          )}
          <span className={s.summarySpacer} />
          {total > PAGE_SIZE && (
            <div className={s.pager}>
              <Button
                size="sm"
                variant="outline"
                icon={ChevronLeft}
                onClick={() => setPage((p) => Math.max(0, p - 1))}
                disabled={page === 0}
              >
                이전
              </Button>
              <span className={s.pageInfo}>
                {page + 1} / {lastPage + 1}
              </span>
              <Button
                size="sm"
                variant="outline"
                onClick={() => setPage((p) => Math.min(lastPage, p + 1))}
                disabled={page >= lastPage}
              >
                다음
                <ChevronRight size={14} />
              </Button>
            </div>
          )}
        </div>
      </div>

      {rows.length === 0 && !list.isFetching ? (
        <Empty
          icon={Users}
          title={hasFilter ? '조건에 맞는 학생이 없습니다' : '아직 등록된 학생이 없습니다'}
          description={
            hasFilter
              ? '검색어나 조건을 바꿔 보세요.'
              : '오른쪽 위 [학생 등록]으로 한 명씩 넣거나, Phase 2에서 만들 Excel 가져오기로 한 번에 넣을 수 있습니다.'
          }
          action={
            hasFilter ? (
              <Button variant="outline" onClick={resetFilters}>
                조건 지우기
              </Button>
            ) : (
              <Button variant="primary" icon={UserPlus} onClick={() => setOpenStudent(null)}>
                학생 등록
              </Button>
            )
          }
        />
      ) : (
        <TableWrap>
          <thead>
            <tr>
              <th style={{ width: 62 }}>학년</th>
              <th style={{ width: 88 }}>반</th>
              <th style={{ width: 56 }} className={tableClass.num}>
                번호
              </th>
              <th style={{ width: 120 }}>이름</th>
              <th style={{ width: 48 }} className={tableClass.center}>
                성별
              </th>
              <th style={{ width: 92 }}>생년월일</th>
              <th>주소</th>
              <th style={{ width: 92 }}>주소 분류</th>
              <th style={{ width: 130 }}>주보호자</th>
              <th style={{ width: 132 }}>상태</th>
            </tr>
          </thead>
          <tbody>
            {list.isFetching && rows.length === 0 && (
              <tr className={s.loadingRow}>
                <td colSpan={10}>불러오는 중…</td>
              </tr>
            )}
            {rows.map((r) => {
              const st = statusBadge(r)
              const ib = issueBadge(r)
              return (
                <tr
                  key={r.id}
                  className={openStudent === r.id ? tableClass.rowActive : undefined}
                  onClick={() => setOpenStudent(r.id)}
                >
                  <td>{r.grade}학년</td>
                  <td>{r.className ?? <span className={tableClass.muted}>미정</span>}</td>
                  <td className={tableClass.num}>{r.classNo ?? ''}</td>
                  <td className={s.nameCell}>
                    {r.issueCount > 0 && <span className={s.issueDot} aria-hidden="true" />}
                    {r.name}
                  </td>
                  <td className={tableClass.center}>{genderLabel(r.gender)}</td>
                  <td>{birthCell(r.birthDate, r.birthRaw)}</td>
                  <td className={s.addrCell} title={r.addressRaw ?? ''}>
                    {r.addressRaw ?? ''}
                  </td>
                  <td>
                    {r.addressCategory ?? <span className={tableClass.muted}>미분류</span>}
                  </td>
                  <td>{r.primaryPhone ?? ''}</td>
                  <td>
                    <Badge tone={st.tone}>{st.label}</Badge>
                    {ib && (
                      <>
                        {' '}
                        <Badge tone={ib.tone}>{ib.label}</Badge>
                      </>
                    )}
                  </td>
                </tr>
              )
            })}
          </tbody>
        </TableWrap>
      )}

      {openStudent !== undefined && (
        <StudentDrawer
          studentId={openStudent}
          schoolYear={schoolYear}
          classSuggestions={suggestions}
          onClose={() => setOpenStudent(undefined)}
          onSaved={refresh}
        />
      )}
    </Page>
  )
}
