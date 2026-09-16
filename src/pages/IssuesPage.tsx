import { useEffect, useMemo, useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import {
  CheckCircle2,
  ChevronLeft,
  ChevronRight,
  RefreshCw,
  Search,
  X,
} from 'lucide-react'

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
  type Tone,
} from '@/components/ui'
import { SiblingBatch } from '@/features/student/SiblingBatch'
import { StudentDrawer } from '@/features/student/StudentDrawer'
import { issueApi, type IssueFilter, type IssueListRow } from '@/ipc/issue'
import { settingsApi } from '@/ipc/settings'
import { studentApi } from '@/ipc/student'
import { rangeLabel } from '@/lib/format'
import { yearLabel } from '@/lib/schoolYear'
import s from './IssuesPage.module.css'

const PAGE_SIZE = 100

const STATUS_OPTIONS = [
  { value: 'OPEN', label: '미해결' },
  { value: 'RESOLVED', label: '해결됨' },
  { value: 'ALL', label: '전체' },
]

/**
 * 종류마다 표시 색을 다르게 하되, **색만으로 뜻을 전하지 않는다.**
 * 뱃지 안에 언제나 한국어 이름이 함께 있다.
 */
const KIND_TONE: Record<string, Tone> = {
  MISSING: 'warn',
  BIRTH: 'warn',
  ADDRESS: 'info',
  NUMBER_DUP: 'error',
  CLASS_ASSIGN: 'warn',
  SIBLING_CANDIDATE: 'info',
  GUARDIAN_FILL: 'info',
  GUARDIAN_CONFLICT: 'error',
  DUPLICATE: 'error',
}

/** `2026-09-15 14:02:11` → `09-15` */
function shortDate(v: string | null): string {
  if (!v) return ''
  const m = v.match(/^\d{4}-(\d{2}-\d{2})/)
  return m ? m[1] : v.slice(0, 10)
}

export function IssuesPage() {
  const qc = useQueryClient()
  const settings = useQuery({ queryKey: ['settings'], queryFn: settingsApi.get })
  const schoolYear = settings.data?.currentYear ?? null

  const [grade, setGrade] = useState<number | null>(null)
  const [className, setClassName] = useState<string | null>(null)
  const [kind, setKind] = useState<string>('')
  const [status, setStatus] = useState<string>('OPEN')
  const [q, setQ] = useState('')
  const [page, setPage] = useState(0)
  const [open, setOpen] = useState<{ studentId: number; tab: string } | null>(null)

  const classOptions = useQuery({
    queryKey: ['class-options', schoolYear],
    queryFn: () => studentApi.classOptions(schoolYear!),
    enabled: schoolYear != null,
  })

  const summary = useQuery({
    queryKey: ['issue-summary', schoolYear],
    queryFn: () => issueApi.summary(schoolYear!),
    enabled: schoolYear != null,
  })

  const filter: IssueFilter | null = useMemo(() => {
    if (schoolYear == null) return null
    return {
      schoolYear,
      grade,
      className,
      kind: kind || null,
      status,
      q: q.trim() || null,
    }
  }, [schoolYear, grade, className, kind, status, q])

  useEffect(() => {
    setPage(0)
  }, [grade, className, kind, status, q])

  const list = useQuery({
    queryKey: ['issues', filter, page],
    queryFn: () => issueApi.list(filter!, PAGE_SIZE, page * PAGE_SIZE),
    enabled: filter != null,
    placeholderData: (prev) => prev,
  })

  const refresh = () => {
    qc.invalidateQueries({ queryKey: ['issues'] })
    qc.invalidateQueries({ queryKey: ['issue-summary'] })
    qc.invalidateQueries({ queryKey: ['students'] })
    qc.invalidateQueries({ queryKey: ['siblings'] })
    qc.invalidateQueries({ queryKey: ['sibling-candidates'] })
  }

  const recompute = useMutation({
    mutationFn: () => issueApi.recompute(schoolYear!),
    onSuccess: refresh,
  })

  const rows = list.data?.rows ?? []
  const total = list.data?.total ?? 0
  const lastPage = Math.max(0, Math.ceil(total / PAGE_SIZE) - 1)
  const classesForGrade = (classOptions.data ?? []).filter(
    (c) => grade == null || c.grade === grade,
  )
  const hasFilter =
    grade != null || className != null || kind !== '' || status !== 'OPEN' || q.trim() !== ''

  const resetFilters = () => {
    setGrade(null)
    setClassName(null)
    setKind('')
    setStatus('OPEN')
    setQ('')
  }

  if (schoolYear == null) {
    return (
      <Page title="확인 필요">
        <Empty
          icon={CheckCircle2}
          title="먼저 학년도를 만들어 주세요"
          description="설정 화면에서 현재 학년도를 지정하면 확인할 항목을 모아 볼 수 있습니다."
        />
      </Page>
    )
  }

  const openTotal = summary.data?.total ?? 0

  return (
    <Page
      title="확인 필요"
      year={yearLabel(schoolYear)}
      actions={
        <Button
          variant="outline"
          icon={RefreshCw}
          onClick={() => recompute.mutate()}
          disabled={recompute.isPending}
        >
          {recompute.isPending ? '다시 계산 중…' : '다시 계산'}
        </Button>
      }
    >
      <ErrorNotice error={list.error ?? summary.error ?? recompute.error} />

      <div className={s.head}>
        <div className={s.total}>
          <span className={s.totalNum}>{openTotal.toLocaleString('ko-KR')}</span>
          <span className={s.totalWord}>건</span>
          <span className={s.totalHint}>
            {openTotal === 0
              ? '지금 확인할 것이 없습니다.'
              : '값을 고쳐 저장하면 해당 항목은 저절로 사라집니다.'}
          </span>
        </div>

        {(summary.data?.byKind.length ?? 0) > 0 && (
          <div className={s.kinds}>
            <button
              type="button"
              className={kind === '' ? `${s.kind} ${s.kindOn}` : s.kind}
              onClick={() => setKind('')}
            >
              전체
              <span className={s.kindNum}>{openTotal.toLocaleString('ko-KR')}</span>
            </button>
            {summary.data?.byKind.map((k) => (
              <button
                key={k.kind}
                type="button"
                className={kind === k.kind ? `${s.kind} ${s.kindOn}` : s.kind}
                onClick={() => setKind(kind === k.kind ? '' : k.kind)}
                title={
                  k.kind === 'SIBLING_CANDIDATE'
                    ? '형제 후보를 골라 한 번에 확정할 수 있습니다'
                    : undefined
                }
              >
                {k.kindLabel}
                <span className={s.kindNum}>{k.count.toLocaleString('ko-KR')}</span>
              </button>
            ))}
          </div>
        )}
      </div>

      <div className={s.filters}>
        <div className={s.filterRow}>
          <div className={s.search}>
            <Search size={16} className={s.searchIcon} />
            <Input
              className={s.searchInput}
              value={q}
              onChange={(e) => setQ(e.target.value)}
              placeholder="학생 이름이나 3-나리 홍길동 처럼 찾기"
            />
            {q && (
              <button
                type="button"
                className={s.clearBtn}
                onClick={() => setQ('')}
                aria-label="지우기"
              >
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
            onChange={(e) => setStatus(e.target.value)}
          >
            {STATUS_OPTIONS.map((o) => (
              <option key={o.value} value={o.value}>
                {o.label}
              </option>
            ))}
          </Select>

          {hasFilter && (
            <Button size="sm" variant="ghost" onClick={resetFilters}>
              조건 지우기
            </Button>
          )}
        </div>

        <div className={s.summary}>
          <span className={s.summaryCount}>
            {rangeLabel(total, page * PAGE_SIZE, rows.length).replace('명', '건')}
          </span>
          {recompute.isSuccess && !recompute.isPending && (
            <span className={s.summaryCount}>
              학생 {recompute.data.toLocaleString('ko-KR')}명을 다시 살펴봤습니다.
            </span>
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

      {/*
        형제 후보만 모아 볼 때는 한 번에 확정할 수 있게 한다. 후보마다 학생 상세를
        열어 [형제로 확인] 을 누르면 학년 전체에서는 감당할 수 없는 작업량이 된다.
      */}
      {kind === 'SIBLING_CANDIDATE' && (
        <SiblingBatch schoolYear={schoolYear} onChanged={refresh} />
      )}

      {rows.length === 0 && !list.isFetching ? (
        <Empty
          icon={CheckCircle2}
          title={hasFilter ? '조건에 맞는 항목이 없습니다' : '확인할 것이 없습니다'}
          description={
            hasFilter
              ? '조건을 바꾸거나 [해결됨]으로 지난 항목을 볼 수 있습니다.'
              : '학생 자료가 모두 채워져 있습니다.'
          }
          action={
            hasFilter ? (
              <Button variant="outline" onClick={resetFilters}>
                조건 지우기
              </Button>
            ) : undefined
          }
        />
      ) : (
        <TableWrap>
          <thead>
            <tr>
              <th style={{ width: 78 }}>상태</th>
              <th style={{ width: 150 }}>학생</th>
              <th style={{ width: 132 }}>종류</th>
              <th>확인할 내용</th>
              <th style={{ width: 74 }}>발생</th>
              <th style={{ width: 74 }}>해결</th>
            </tr>
          </thead>
          <tbody>
            {list.isFetching && rows.length === 0 && (
              <tr className={s.loadingRow}>
                <td colSpan={6}>불러오는 중…</td>
              </tr>
            )}
            {rows.map((r) => (
              <Row
                key={r.id}
                row={r}
                active={open?.studentId === r.studentId}
                onOpen={() => setOpen({ studentId: r.studentId, tab: r.tab })}
              />
            ))}
          </tbody>
        </TableWrap>
      )}

      <div className={s.note}>
        <Notice tone="info">
          [다시 계산]은 학생 자료를 다시 살펴 표시를 맞춥니다. 주소 규칙을 새로 적용하거나 형제
          후보를 다시 찾으려면 <b>주소 규칙</b> 화면과 <b>설정</b> 화면의 전용 버튼을 써 주세요.
        </Notice>
      </div>

      {open && (
        <StudentDrawer
          studentId={open.studentId}
          schoolYear={schoolYear}
          classSuggestions={Array.from(
            new Set((classOptions.data ?? []).map((c) => c.className)),
          )}
          initialTab={open.tab === 'sibling' ? 'sibling' : 'basic'}
          onClose={() => setOpen(null)}
          onSaved={refresh}
        />
      )}
    </Page>
  )
}

function Row({
  row,
  active,
  onOpen,
}: {
  row: IssueListRow
  active: boolean
  onOpen: () => void
}) {
  const resolved = row.status !== 'OPEN'
  return (
    <tr className={active ? tableClass.rowActive : undefined} onClick={onOpen}>
      <td>
        <Badge tone={resolved ? 'success' : 'warn'}>{row.statusLabel}</Badge>
      </td>
      <td className={s.studentCell}>
        {row.studentLabel}
        {row.classNo != null && <span className={tableClass.muted}> {row.classNo}번</span>}
      </td>
      <td>
        <Badge tone={KIND_TONE[row.kind] ?? 'neutral'}>{row.kindLabel}</Badge>
      </td>
      <td className={s.msgCell} title={row.message}>
        <span className={s.msg}>{row.message}</span>
        {row.detail && !row.detail.startsWith('{') && !row.detail.startsWith('[') && (
          <span className={s.detail}>{row.detail}</span>
        )}
      </td>
      <td className={s.whenCell}>{shortDate(row.createdAt)}</td>
      <td className={s.whenCell}>{shortDate(row.resolvedAt)}</td>
    </tr>
  )
}
