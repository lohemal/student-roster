import { useMemo, useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { useNavigate } from 'react-router-dom'
import { BarChart3 } from 'lucide-react'

import { Button, Empty, ErrorNotice, Notice, Page, Select } from '@/components/ui'
import { addressApi } from '@/ipc/address'
import { settingsApi } from '@/ipc/settings'
import {
  statsApi,
  type AddressRow,
  type Counts,
  type StatFilter,
  type StatsOverview,
} from '@/ipc/stats'
import { yearLabel } from '@/lib/schoolYear'
import s from './StatsPage.module.css'

/** `1,054` */
const n = (v: number) => v.toLocaleString('ko-KR')

/**
 * 주소 조건을 하나의 고르기 값으로 다룬다.
 *
 * `''` 전체 / `NONE` 미분류 / `NOADDR` 주소 없음 / 숫자 분류 id.
 * 학생명단이 쓰는 값과 같은 모양이라 그대로 넘겨 줄 수 있다.
 */
function addressFilter(pick: string): Partial<StatFilter> {
  if (pick === '') return {}
  if (pick === 'NONE') return { addressUnclassified: true }
  if (pick === 'NOADDR') return { addressNone: true }
  return { addressCategoryId: Number(pick) }
}

/**
 * 통계 화면.
 *
 * **학생명단에 보이는 인원과 여기 숫자가 반드시 같아야 한다.** 그래서 세는 기준을
 * 따로 만들지 않고 명단·전입 반 배정과 같은 `repo::stats` 를 쓴다. 집계 결과를
 * 저장해 두지도 않는다 — 언제나 지금 자료를 센다.
 *
 * 화려한 대시보드가 아니라 **숫자를 견주는 표**가 목적이다.
 */
export function StatsPage() {
  const nav = useNavigate()
  const settings = useQuery({ queryKey: ['settings'], queryFn: settingsApi.get })
  const currentYear = settings.data?.currentYear ?? null

  const [year, setYear] = useState<number | null>(null)
  const [grade, setGrade] = useState<number | null>(null)
  const [addressPick, setAddressPick] = useState('')

  const schoolYear = year ?? currentYear

  const cats = useQuery({
    queryKey: ['address-categories', schoolYear],
    queryFn: () => addressApi.categories(schoolYear!),
    enabled: schoolYear != null,
  })

  const filter: StatFilter | null = useMemo(() => {
    if (schoolYear == null) return null
    return { schoolYear, grade, ...addressFilter(addressPick) }
  }, [schoolYear, grade, addressPick])

  const q = useQuery({
    queryKey: ['stats', filter],
    queryFn: () => statsApi.overview(filter!),
    enabled: filter != null,
    placeholderData: (prev) => prev,
  })

  if (schoolYear == null) {
    return (
      <Page title="통계">
        <Empty
          icon={BarChart3}
          title="먼저 학년도를 만들어 주세요"
          description="설정 화면에서 현재 학년도를 지정하면 학생 수를 볼 수 있습니다."
        />
      </Page>
    )
  }

  const data = q.data
  const picked = cats.data?.find((c) => String(c.id) === addressPick)

  /** 지금 조건 그대로 학생명단을 연다 */
  const openRoster = (extra: { grade?: number | null; address?: string } = {}) => {
    const p = new URLSearchParams()
    const g = extra.grade !== undefined ? extra.grade : grade
    if (g != null) p.set('grade', String(g))
    const a = extra.address !== undefined ? extra.address : addressPick
    if (a) p.set('address', a)
    nav(`/students?${p.toString()}`)
  }

  /** 모집단을 한 줄로 — 조건을 걸어도 무엇을 센 숫자인지 알 수 있게 */
  const who = [
    grade != null ? `${grade}학년` : null,
    addressPick === 'NONE'
      ? '주소 미분류'
      : addressPick === 'NOADDR'
        ? '주소 없음'
        : (picked?.name ?? null),
  ]
    .filter(Boolean)
    .join(' · ')

  return (
    <Page title="통계" year={yearLabel(schoolYear)}>
      <ErrorNotice error={q.error ?? settings.error} />

      <div className={s.filters}>
        <Select
          className={s.compact}
          value={schoolYear}
          onChange={(e) => setYear(Number(e.target.value))}
          aria-label="학년도"
        >
          {(settings.data?.years ?? []).map((y) => (
            <option key={y.year} value={y.year}>
              {y.year}학년도
            </option>
          ))}
        </Select>

        <Select
          className={s.compact}
          value={grade ?? ''}
          onChange={(e) => setGrade(e.target.value === '' ? null : Number(e.target.value))}
          aria-label="학년"
        >
          <option value="">전체 학년</option>
          {(data?.address.grades ?? []).map((g) => (
            <option key={g} value={g}>
              {g}학년
            </option>
          ))}
          {/* 고른 학년에 학생이 없어도 고른 값은 남아 있어야 한다 */}
          {grade != null && !(data?.address.grades ?? []).includes(grade) && (
            <option value={grade}>{grade}학년</option>
          )}
        </Select>

        <Select
          className={s.compact}
          value={addressPick}
          onChange={(e) => setAddressPick(e.target.value)}
          aria-label="주소 분류"
        >
          <option value="">전체 주소</option>
          {(cats.data ?? []).map((c) => (
            <option key={c.id} value={String(c.id)}>
              {c.name}
            </option>
          ))}
          <option value="NONE">미분류</option>
          <option value="NOADDR">주소 없음</option>
        </Select>

        {(grade != null || addressPick !== '') && (
          <Button
            size="sm"
            variant="ghost"
            onClick={() => {
              setGrade(null)
              setAddressPick('')
            }}
          >
            조건 지우기
          </Button>
        )}

        <span className={s.filterSpacer} />
        {data && (
          <span className={s.basis}>
            {data.isCurrentYear ? '현재 재학생 기준' : '학년도 최종 재적 기준'}
          </span>
        )}
      </div>

      {!data ? (
        <div className={s.basis}>불러오는 중…</div>
      ) : data.totals.total === 0 ? (
        <Empty
          icon={BarChart3}
          title="셀 학생이 없습니다"
          description="조건을 바꾸거나 학생을 먼저 등록해 주세요."
        />
      ) : (
        <>
          <Summary data={data} who={who} />

          <Section
            title="학년별"
            hint="학생이 있는 학년만 나옵니다"
            action={<Button size="sm" variant="ghost" onClick={() => openRoster()}>학생명단 열기</Button>}
          >
            <Table
              head={['학년', '남', '여', '미입력', '합계']}
              rows={data.byGrade.map((r) => ({
                key: String(r.grade),
                name: `${r.grade}학년`,
                counts: r,
                onOpen: () => openRoster({ grade: r.grade }),
              }))}
              total={data.totals}
            />
          </Section>

          <Section title="학년 · 반별" hint="반이 정해지지 않은 학생은 미정으로 셉니다">
            <Table
              head={['학년 · 반', '남', '여', '미입력', '합계']}
              rows={data.byClass.map((r) => ({
                key: r.classLabel,
                name: r.classLabel,
                counts: r,
                onOpen: () => openRoster({ grade: r.grade }),
              }))}
              total={data.totals}
            />
          </Section>

          <Section
            title="주소 분류"
            hint="기타 · 미분류 · 주소 없음은 서로 다른 상태입니다"
          >
            <AddressCross
              data={data}
              onOpen={(row, g) =>
                openRoster({
                  grade: g,
                  address:
                    row.bucket === 'CATEGORY'
                      ? String(row.categoryId)
                      : row.bucket === 'UNCLASSIFIED'
                        ? 'NONE'
                        : 'NOADDR',
                })
              }
            />
            <Quality data={data} />
          </Section>

          <div className={s.foot}>
            {data.consistency.ok ? (
              <span className={s.checked}>
                이 화면의 모든 표는 같은 {n(data.consistency.total)}명을 센 것입니다. 학년 ·
                반 · 성별 · 주소 합계가 서로 맞는지 볼 때마다 확인합니다.
              </span>
            ) : (
              <Notice tone="error">
                표마다 합계가 다릅니다. 전체 {n(data.consistency.total)} · 성별{' '}
                {n(data.consistency.genderSum)} · 학년 {n(data.consistency.gradeSum)} · 반{' '}
                {n(data.consistency.classSum)} · 주소 {n(data.consistency.addressSum)}. 이
                화면의 숫자를 믿지 마시고 알려 주세요.
              </Notice>
            )}
          </div>
        </>
      )}
    </Page>
  )
}

function Summary({ data, who }: { data: StatsOverview; who: string }) {
  const t = data.totals
  return (
    <div className={s.summary}>
      <span className={s.summaryWho}>{who || '전체'}</span>
      <span className={s.summaryNum}>{n(t.total)}</span>
      <span className={s.summaryWord}>명</span>
      <span className={s.summaryParts}>
        <span className={s.summaryPart}>
          남 <b>{n(t.male)}</b>
        </span>
        <span className={s.summaryPart}>
          여 <b>{n(t.female)}</b>
        </span>
        <span className={s.summaryPart}>
          성별 미입력 <b>{n(t.unknown)}</b>
        </span>
      </span>
    </div>
  )
}

function Section({
  title,
  hint,
  action,
  children,
}: {
  title: string
  hint?: string
  action?: React.ReactNode
  children: React.ReactNode
}) {
  return (
    <section className={s.section}>
      <div className={s.sectionHead}>
        <h2 className={s.sectionTitle}>{title}</h2>
        {hint && <span className={s.sectionHint}>{hint}</span>}
        <span className={s.sectionSpacer} />
        {action}
      </div>
      {children}
    </section>
  )
}

interface Row {
  key: string
  name: string
  counts: Counts
  onOpen?: () => void
}

function Table({ head, rows, total }: { head: string[]; rows: Row[]; total: Counts }) {
  return (
    <div className={s.tableWrap}>
      <table className={s.table}>
        <thead>
          <tr>
            {head.map((h, i) => (
              <th key={h} className={i === 0 ? s.sticky : s.num}>
                {h}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.key}>
              <td
                className={`${s.sticky} ${s.groupName} ${r.onOpen ? s.linkCell : ''}`}
                onClick={r.onOpen}
                title={r.onOpen ? '학생명단에서 보기' : undefined}
              >
                {r.name}
              </td>
              <td className={s.num}>{n(r.counts.male)}</td>
              <td className={s.num}>{n(r.counts.female)}</td>
              <td className={s.num}>{n(r.counts.unknown)}</td>
              <td className={s.num}>{n(r.counts.total)}</td>
            </tr>
          ))}
          <tr className={s.totalRow}>
            <td className={s.sticky}>합계</td>
            <td className={s.num}>{n(total.male)}</td>
            <td className={s.num}>{n(total.female)}</td>
            <td className={s.num}>{n(total.unknown)}</td>
            <td className={s.num}>{n(total.total)}</td>
          </tr>
        </tbody>
      </table>
    </div>
  )
}

/** 주소 분류 × 학년 교차표 */
function AddressCross({
  data,
  onOpen,
}: {
  data: StatsOverview
  onOpen: (row: AddressRow, grade: number | null) => void
}) {
  const { grades, rows, gradeTotals, total } = data.address
  return (
    <div className={s.tableWrap}>
      <table className={s.table}>
        <thead>
          <tr>
            <th className={s.sticky}>주소 분류</th>
            {grades.map((g) => (
              <th key={g} className={s.num}>
                {g}학년
              </th>
            ))}
            <th className={s.num}>합계</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={`${r.bucket}-${r.categoryId ?? r.name}`}>
              <td
                className={`${s.sticky} ${s.groupName} ${s.linkCell}`}
                onClick={() => onOpen(r, null)}
                title="학생명단에서 보기"
              >
                {r.name}
              </td>
              {r.byGrade.map((v, i) => (
                <td
                  key={grades[i]}
                  className={`${s.num} ${v > 0 ? s.linkCell : s.zero}`}
                  onClick={v > 0 ? () => onOpen(r, grades[i]) : undefined}
                  title={v > 0 ? `${grades[i]}학년 ${r.name} 학생 보기` : undefined}
                >
                  {n(v)}
                </td>
              ))}
              <td className={s.num}>{n(r.total)}</td>
            </tr>
          ))}
          <tr className={s.totalRow}>
            <td className={s.sticky}>합계</td>
            {gradeTotals.map((v, i) => (
              <td key={grades[i]} className={s.num}>
                {n(v)}
              </td>
            ))}
            <td className={s.num}>{n(total)}</td>
          </tr>
        </tbody>
      </table>
    </div>
  )
}

function Quality({ data }: { data: StatsOverview }) {
  const q = data.addressQuality
  return (
    <div className={s.quality}>
      <span className={s.qualityItem}>
        <span className={s.qualityKey}>주소 분류 완료</span>
        <span className={s.qualityVal}>
          {n(q.classified)} / {n(q.total)}
        </span>
      </span>
      <span className={s.qualityItem}>
        <span className={s.qualityKey}>미분류</span>
        <span className={s.qualityVal}>{n(q.unclassified)}</span>
      </span>
      <span className={s.qualityItem}>
        <span className={s.qualityKey}>주소 없음</span>
        <span className={s.qualityVal}>{n(q.noAddress)}</span>
      </span>
      {q.conflict > 0 && (
        <span className={s.qualityItem}>
          <span className={s.qualityKey}>규칙 충돌</span>
          <span className={s.qualityVal}>{n(q.conflict)}</span>
        </span>
      )}
      <span className={s.qualityItem}>
        <span className={s.qualityKey}>직접 지정</span>
        <span className={s.qualityVal}>{n(q.manual)}</span>
      </span>
    </div>
  )
}
