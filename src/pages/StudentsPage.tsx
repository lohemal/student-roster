import { useEffect, useMemo, useState } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useNavigate, useSearchParams } from 'react-router-dom'
import {
  ChevronLeft,
  ChevronRight,
  Download,
  ListFilter as FilterIcon,
  FileSpreadsheet,
  Phone,
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
  Notice,
  Field,
  Input,
  Select,
  TableWrap,
  tableClass,
  Page,
} from '@/components/ui'
import {
  PrimaryFillDialog,
  PrimaryPhoneCell,
} from '@/features/student/PrimaryPhone'
import { StudentDrawer } from '@/features/student/StudentDrawer'
import { addressApi } from '@/ipc/address'
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
  const nav = useNavigate()
  const settings = useQuery({ queryKey: ['settings'], queryFn: settingsApi.get })
  const schoolYear = settings.data?.currentYear ?? null

  // 통계 화면에서 숫자를 누르고 넘어오면 그 조건으로 명단을 연다.
  // 통계 전용 목록을 따로 만들지 않고 이 화면 하나를 쓴다.
  const [params] = useSearchParams()

  const [q, setQ] = useState('')
  const [grade, setGrade] = useState<number | null>(() => {
    const g = Number(params.get('grade'))
    return Number.isFinite(g) && g >= 1 && g <= 6 ? g : null
  })
  const [className, setClassName] = useState<string | null>(null)
  const [status, setStatus] = useState<StatusFilter>('ACTIVE')
  // '' 전체 / 'NONE' 미분류만 / 'NOADDR' 주소 없음 / 숫자 분류 id
  const [addressCat, setAddressCat] = useState<string>(() => params.get('address') ?? '')
  const [onlyIssues, setOnlyIssues] = useState(false)
  const [showAdvanced, setShowAdvanced] = useState(false)
  const [adv, setAdv] = useState<Advanced>(EMPTY_ADVANCED)
  const [page, setPage] = useState(0)

  // 열려 있는 학생. undefined = 닫힘, null = 새 등록
  const [openStudent, setOpenStudent] = useState<number | null | undefined>(undefined)
  /** 주보호자 연락처 일괄 설정 창 */
  const [fillOpen, setFillOpen] = useState(false)
  const [done, setDone] = useState<string | null>(null)

  const addressCats = useQuery({
    queryKey: ['address-categories', schoolYear],
    queryFn: () => addressApi.categories(schoolYear!),
    enabled: schoolYear != null,
  })

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
      addressCategoryId:
        addressCat && addressCat !== 'NONE' && addressCat !== 'NOADDR'
          ? Number(addressCat)
          : null,
      addressUnclassified: addressCat === 'NONE',
      addressNone: addressCat === 'NOADDR',
      name: adv.name.trim() || undefined,
      classNo: adv.classNo.trim() ? Number(adv.classNo) : null,
      address: adv.address.trim() || undefined,
      fatherPhone: adv.fatherPhone.trim() || undefined,
      motherPhone: adv.motherPhone.trim() || undefined,
      primaryPhone: adv.primaryPhone.trim() || undefined,
    }
  }, [schoolYear, q, grade, className, status, onlyIssues, adv, addressCat])

  // 조건이 바뀌면 첫 쪽으로
  useEffect(() => {
    setPage(0)
  }, [q, grade, className, status, onlyIssues, adv, addressCat])

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
    qc.invalidateQueries({ queryKey: ['primary-fill'] })
    qc.invalidateQueries({ queryKey: ['class-options'] })
    qc.invalidateQueries({ queryKey: ['issue-summary'] })
    qc.invalidateQueries({ queryKey: ['issues'] })
    qc.invalidateQueries({ queryKey: ['address-categories'] })
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
    addressCat !== '' ||
    status !== 'ACTIVE' ||
    Object.values(adv).some((v) => v.trim() !== '')

  const resetFilters = () => {
    setQ('')
    setGrade(null)
    setClassName(null)
    setStatus('ACTIVE')
    setOnlyIssues(false)
    setAddressCat('')
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
        <>
          <Button
            variant="outline"
            icon={FileSpreadsheet}
            onClick={() => nav('/students/import')}
          >
            가져오기
          </Button>
          {/* 지금 걸어 둔 조건 그대로 내보내기 화면으로. 조건을 두 번 만들지 않는다. */}
          <Button
            variant="outline"
            icon={Download}
            onClick={() => nav('/export', { state: { filter } })}
          >
            내보내기
          </Button>
          <Button variant="outline" icon={Phone} onClick={() => setFillOpen(true)}>
            주보호자 일괄 설정
          </Button>
          <Button variant="primary" icon={UserPlus} onClick={() => setOpenStudent(null)}>
            학생 등록
          </Button>
        </>
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

          <Select
            className={s.compact}
            value={addressCat}
            onChange={(e) => setAddressCat(e.target.value)}
          >
            <option value="">전체 주소</option>
            <option value="NONE">미분류</option>
            <option value="NOADDR">주소 없음</option>
            {(addressCats.data ?? []).map((c) => (
              <option key={c.id} value={String(c.id)}>
                {c.name}
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

      {done && (
        <div className={s.doneRow}>
          <Notice tone="success">{done}</Notice>
        </div>
      )}

      {rows.length === 0 && !list.isFetching ? (
        <Empty
          icon={Users}
          title={hasFilter ? '조건에 맞는 학생이 없습니다' : '아직 등록된 학생이 없습니다'}
          description={
            hasFilter
              ? '검색어나 조건을 바꿔 보세요.'
              : '오른쪽 위 [학생 등록]으로 한 명씩 넣거나, [가져오기]로 엑셀 명단을 한 번에 넣을 수 있습니다.'
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
              <th style={{ width: 164 }}>주보호자</th>
              <th style={{ width: 112 }}>본교 형제</th>
              <th style={{ width: 132 }}>상태</th>
            </tr>
          </thead>
          <tbody>
            {list.isFetching && rows.length === 0 && (
              <tr className={s.loadingRow}>
                <td colSpan={11}>불러오는 중…</td>
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
                  <td>
                    <PrimaryPhoneCell row={r} />
                  </td>
                  {/* 형제가 둘 이상이어도 전원 보여 준다 — 상세를 열지 않고 확인한다 */}
                  <td className={s.siblingCell} title={r.sibling?.text}>
                    {r.sibling ? r.sibling.text : ''}
                  </td>
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

      {fillOpen && filter && (
        <PrimaryFillDialog
          filter={filter}
          filtered={total}
          hasFilter={hasFilter}
          onClose={() => setFillOpen(false)}
          onDone={(msg) => {
            setFillOpen(false)
            setDone(msg)
            refresh()
          }}
        />
      )}
    </Page>
  )
}
