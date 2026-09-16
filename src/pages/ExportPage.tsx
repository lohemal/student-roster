import { useEffect, useMemo, useState } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useLocation, useNavigate } from 'react-router-dom'
import { open, save } from '@tauri-apps/plugin-dialog'
import {
  ArrowDown,
  ArrowUp,
  CheckCircle2,
  FileSpreadsheet,
  FolderOpen,
  X,
} from 'lucide-react'

import {
  Button,
  Card,
  Empty,
  ErrorNotice,
  Notice,
  Page,
  Select,
  TableWrap,
  tableClass,
} from '@/components/ui'
import { StudentDrawer } from '@/features/student/StudentDrawer'
import { addressApi } from '@/ipc/address'
import {
  exportApi,
  EXPORT_EXISTS,
  type ExportPreset,
  type ExportRequest,
  type ExportResult,
  type ExportWarning,
  type Grouping,
} from '@/ipc/export'
import { isAppError } from '@/ipc/invoke'
import { settingsApi } from '@/ipc/settings'
import { studentApi, type ListFilter } from '@/ipc/student'
import { yearLabel } from '@/lib/schoolYear'
import s from './ExportPage.module.css'

const n = (v: number) => v.toLocaleString('ko-KR')

/** 미리보기에서 사람 이름을 몇 명까지 늘어놓을지 */
const MAX_NAMES = 12

const PRESETS: { key: ExportPreset; title: string; desc: string }[] = [
  {
    key: 'CUSTOM',
    title: '사용자 지정 Excel',
    desc: '필요한 열만 골라 원하는 차례로 만듭니다.',
  },
  {
    key: 'SCHOOLJONGI',
    title: '학교종이 학생명단',
    desc: '학급마다 시트 한 장. 번호·이름·보호자 연락처를 양식대로 채웁니다.',
  },
  {
    key: 'ALIME',
    title: '알림e 문자서비스',
    desc: '이름·전화번호·비고 세 열. 한 파일에 1,000명까지 담습니다.',
  },
]

const GROUPINGS: { key: Grouping; label: string; desc: string }[] = [
  { key: 'ALL', label: '전체', desc: '한 파일로 (1,000명을 넘으면 나눕니다)' },
  { key: 'GRADE', label: '학년별', desc: '학생이 있는 학년마다 한 파일' },
  { key: 'GRADE_CLASS', label: '학년·반별', desc: '학급마다 한 파일' },
]

/** 주소 조건 하나를 학생명단·통계와 같은 값으로 다룬다. */
function addressPart(pick: string) {
  return {
    addressCategoryId: pick && pick !== 'NONE' && pick !== 'NOADDR' ? Number(pick) : null,
    addressUnclassified: pick === 'NONE',
    addressNone: pick === 'NOADDR',
  }
}

/** 명단에서 넘어온 조건을 고르기 값으로 되돌린다. */
function pickOf(f?: ListFilter | null): string {
  if (!f) return ''
  if (f.addressUnclassified) return 'NONE'
  if (f.addressNone) return 'NOADDR'
  return f.addressCategoryId ? String(f.addressCategoryId) : ''
}

/** 학년·주소 말고 명단에서 함께 넘어온 조건을 한 줄로 */
function extraLabel(f: ListFilter | null): string[] {
  if (!f) return []
  const out: string[] = []
  if (f.q?.trim()) out.push(`검색 '${f.q.trim()}'`)
  if (f.className) out.push(`${f.className}반`)
  if (f.classNo) out.push(`${f.classNo}번`)
  if (f.name?.trim()) out.push(`이름 '${f.name.trim()}'`)
  if (f.address?.trim()) out.push(`주소 '${f.address.trim()}'`)
  if (f.onlyIssues) out.push('확인 필요만')
  if (f.status && f.status !== 'ACTIVE') out.push(`상태 ${f.status}`)
  return out
}

/**
 * 파일 내보내기.
 *
 * 세 가지 양식이 **하나의 엔진**을 지난다 — 조건으로 학생을 뽑고(필터), 파일로
 * 나누고(묶기), 양식대로 값을 채운다. 그래서 "3학년만, 반별로, 알림e 양식" 같은
 * 조합이 따로 만든 코드 없이 나온다.
 *
 * 만들기 전에 **어떤 파일이 몇 개, 누가 빠지는지**를 먼저 보여 준다.
 */
export function ExportPage() {
  const nav = useNavigate()
  const qc = useQueryClient()
  const loc = useLocation()
  const handoff = (loc.state as { filter?: ListFilter } | null)?.filter ?? null

  const settings = useQuery({ queryKey: ['settings'], queryFn: settingsApi.get })
  const currentYear = settings.data?.currentYear ?? null

  const [fromRoster, setFromRoster] = useState<ListFilter | null>(handoff)
  const [year, setYear] = useState<number | null>(handoff?.schoolYear ?? null)
  const [grade, setGrade] = useState<number | null>(handoff?.grade ?? null)
  const [addressPick, setAddressPick] = useState(pickOf(handoff))
  const [preset, setPreset] = useState<ExportPreset>('CUSTOM')
  const [grouping, setGrouping] = useState<Grouping>('ALL')
  const [picked, setPicked] = useState<string[] | null>(null)
  const [busy, setBusy] = useState(false)
  const [runError, setRunError] = useState<unknown>(null)
  const [result, setResult] = useState<ExportResult | null>(null)
  /** 같은 이름의 파일이 있어 사용자 확인을 기다리는 중 */
  const [askOverwrite, setAskOverwrite] = useState<{ target: string; message: string } | null>(null)
  const [openStudent, setOpenStudent] = useState<number | undefined>(undefined)

  const schoolYear = year ?? currentYear

  const cats = useQuery({
    queryKey: ['address-categories', schoolYear],
    queryFn: () => addressApi.categories(schoolYear!),
    enabled: schoolYear != null,
  })
  const classOptions = useQuery({
    queryKey: ['class-options', schoolYear],
    queryFn: () => studentApi.classOptions(schoolYear!),
    enabled: schoolYear != null,
  })
  const columns = useQuery({ queryKey: ['export-columns'], queryFn: exportApi.columns })

  // 처음 열면 기본 열 구성으로 시작한다
  useEffect(() => {
    if (picked == null && columns.data) {
      setPicked(columns.data.filter((c) => c.defaultOn).map((c) => c.key))
    }
  }, [columns.data, picked])

  const filter: ListFilter | null = useMemo(() => {
    if (schoolYear == null) return null
    return {
      ...(fromRoster ?? {}),
      schoolYear,
      grade,
      ...addressPart(addressPick),
      status: fromRoster?.status ?? 'ACTIVE',
    }
  }, [fromRoster, schoolYear, grade, addressPick])

  const request: ExportRequest | null = useMemo(() => {
    if (!filter) return null
    return {
      preset,
      filter,
      columns: picked ?? [],
      // 학교종이는 학급이 시트로 들어가므로 파일을 나누지 않는다
      grouping: preset === 'SCHOOLJONGI' ? 'ALL' : grouping,
    }
  }, [preset, filter, picked, grouping])

  const ready = request != null && (preset !== 'CUSTOM' || (picked?.length ?? 0) > 0)

  const preview = useQuery({
    queryKey: ['export-preview', request],
    queryFn: () => exportApi.preview(request!),
    enabled: ready,
    placeholderData: (prev) => prev,
  })

  // 조건이 바뀌면 지난 결과는 지운다 — 바뀐 조건과 섞여 보이면 안 된다
  useEffect(() => {
    setResult(null)
    setRunError(null)
    setAskOverwrite(null)
  }, [request])

  const data = preview.data
  const chosen = (picked ?? [])
    .map((k) => columns.data?.find((c) => c.key === k))
    .filter((c): c is NonNullable<typeof c> => c != null)

  const toggleColumn = (key: string) => {
    setPicked((prev) => {
      const cur = prev ?? []
      return cur.includes(key) ? cur.filter((k) => k !== key) : [...cur, key]
    })
  }

  const move = (i: number, dir: -1 | 1) => {
    setPicked((prev) => {
      const cur = [...(prev ?? [])]
      const j = i + dir
      if (j < 0 || j >= cur.length) return cur
      ;[cur[i], cur[j]] = [cur[j], cur[i]]
      return cur
    })
  }

  const doRun = async (target: string, overwrite: boolean) => {
    if (!request) return
    setBusy(true)
    setRunError(null)
    setAskOverwrite(null)
    try {
      setResult(await exportApi.run(request, target, overwrite))
    } catch (e) {
      if (isAppError(e) && e.code === EXPORT_EXISTS) {
        setAskOverwrite({ target, message: e.userMessage })
      } else {
        setRunError(e)
      }
    } finally {
      setBusy(false)
    }
  }

  /** 저장 위치는 **사용자가 고른다.** 찾기 어려운 곳에 말없이 두지 않는다. */
  const start = async () => {
    if (!data || data.files.length === 0) return
    if (data.singleFile) {
      const target = await save({
        defaultPath: `${data.defaultFileName ?? '학생명단'}.xlsx`,
        filters: [{ name: 'Excel 통합 문서', extensions: ['xlsx'] }],
      })
      // 취소는 오류가 아니다
      if (!target) return
      // 저장 창이 이미 덮어쓸지 물었다
      await doRun(target, true)
    } else {
      const target = await open({
        directory: true,
        title: `파일 ${data.files.length}개를 저장할 폴더를 고르세요`,
      })
      if (!target || typeof target !== 'string') return
      await doRun(target, false)
    }
  }

  if (schoolYear == null) {
    return (
      <Page title="파일 내보내기">
        <Empty
          icon={FileSpreadsheet}
          title="먼저 학년도를 만들어 주세요"
          description="설정 화면에서 현재 학년도를 지정하면 명단을 내보낼 수 있습니다."
        />
      </Page>
    )
  }

  const extras = extraLabel(fromRoster)

  return (
    <Page
      title="파일 내보내기"
      year={yearLabel(schoolYear)}
      description="한 번 관리한 학생 자료로 필요한 명단을 그때그때 만듭니다"
    >
      <ErrorNotice error={preview.error ?? runError} />

      {/* 1. 누구를 내보낼지 */}
      <Card title="내보낼 학생" description="학생명단과 같은 조건입니다.">
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
            <option value="">학년 전체</option>
            {Array.from(new Set((classOptions.data ?? []).map((c) => c.grade)))
              .sort((a, b) => a - b)
              .map((g) => (
                <option key={g} value={g}>
                  {g}학년
                </option>
              ))}
          </Select>

          <Select
            value={addressPick}
            onChange={(e) => setAddressPick(e.target.value)}
            aria-label="주소 분류"
          >
            <option value="">주소 전체</option>
            <option value="NONE">미분류만</option>
            <option value="NOADDR">주소 없음</option>
            {(cats.data ?? []).map((c) => (
              <option key={c.id} value={c.id}>
                {c.name}
              </option>
            ))}
          </Select>

          <span className={s.count}>
            내보낼 학생 <strong>{n(data?.matched ?? 0)}명</strong>
          </span>
        </div>

        {extras.length > 0 && (
          <div className={s.handoff}>
            <span>학생명단에서 가져온 조건: {extras.join(' · ')}</span>
            <Button size="sm" variant="ghost" icon={X} onClick={() => setFromRoster(null)}>
              조건 해제
            </Button>
          </div>
        )}
      </Card>

      {/* 2. 어떤 양식으로 */}
      <Card title="양식">
        <div className={s.presets}>
          {PRESETS.map((p) => (
            <button
              key={p.key}
              type="button"
              className={preset === p.key ? `${s.preset} ${s.presetOn}` : s.preset}
              onClick={() => setPreset(p.key)}
              aria-pressed={preset === p.key}
            >
              <span className={s.presetTitle}>{p.title}</span>
              <span className={s.presetDesc}>{p.desc}</span>
            </button>
          ))}
        </div>

        {preset === 'SCHOOLJONGI' && (
          <Notice tone="info">
            보호자휴대폰1은 주보호자 → 모 → 부 차례로, 보호자휴대폰2는 1번에 쓰이지 않은
            모·부 연락처를 넣습니다. <strong>같은 번호를 두 칸에 적지 않습니다.</strong>{' '}
            학생휴대폰은 이 프로그램이 다루지 않는 자료라 빈칸으로 둡니다.
          </Notice>
        )}
        {preset === 'ALIME' && (
          <Notice tone="info">
            전화번호는 주보호자 → 모 → 부 차례로 넣습니다. <strong>비고는 빈칸으로 나갑니다</strong>
            {' '}— 열은 양식대로 그대로 있으니 필요한 내용을 직접 적어 쓰세요.
            이름·전화번호가 15자를 넘으면 <strong>자르지 않고</strong> 먼저 알려 드립니다.
          </Notice>
        )}
      </Card>

      {/* 3-a. 사용자 지정 — 열 고르기 */}
      {preset === 'CUSTOM' && (
        <Card
          title="열 고르기"
          description="고른 차례가 Excel 의 열 차례가 됩니다."
          actions={
            <Button
              size="sm"
              variant="ghost"
              onClick={() =>
                setPicked((columns.data ?? []).filter((c) => c.defaultOn).map((c) => c.key))
              }
            >
              기본 구성으로
            </Button>
          }
        >
          <div className={s.columnPick}>
            {(columns.data ?? []).map((c) => {
              const on = (picked ?? []).includes(c.key)
              return (
                <button
                  key={c.key}
                  type="button"
                  className={on ? `${s.colChip} ${s.colChipOn}` : s.colChip}
                  onClick={() => toggleColumn(c.key)}
                  aria-pressed={on}
                >
                  {c.label}
                  {c.personal && <span className={s.personal}>개인정보</span>}
                </button>
              )
            })}
          </div>

          {chosen.length === 0 ? (
            <Notice tone="warn">내보낼 열을 하나 이상 골라 주세요.</Notice>
          ) : (
            <ol className={s.chosen}>
              {chosen.map((c, i) => (
                <li key={c.key} className={s.chosenItem}>
                  <span className={s.chosenNo}>{i + 1}</span>
                  <span className={s.chosenLabel}>{c.label}</span>
                  <Button
                    size="sm"
                    variant="ghost"
                    icon={ArrowUp}
                    aria-label={`${c.label} 앞으로`}
                    disabled={i === 0}
                    onClick={() => move(i, -1)}
                  />
                  <Button
                    size="sm"
                    variant="ghost"
                    icon={ArrowDown}
                    aria-label={`${c.label} 뒤로`}
                    disabled={i === chosen.length - 1}
                    onClick={() => move(i, 1)}
                  />
                  <Button
                    size="sm"
                    variant="ghost"
                    icon={X}
                    aria-label={`${c.label} 빼기`}
                    onClick={() => toggleColumn(c.key)}
                  />
                </li>
              ))}
            </ol>
          )}
        </Card>
      )}

      {/* 3-b. 파일 생성 단위 */}
      {preset !== 'SCHOOLJONGI' && (
        <Card
          title="파일 생성 단위"
          description="누구를 뽑을지와 몇 개 파일로 나눌지는 다른 이야기입니다."
        >
          <div className={s.groupings}>
            {GROUPINGS.map((g) => (
              <label key={g.key} className={grouping === g.key ? `${s.group} ${s.groupOn}` : s.group}>
                <input
                  type="radio"
                  name="grouping"
                  value={g.key}
                  checked={grouping === g.key}
                  onChange={() => setGrouping(g.key)}
                />
                <span>
                  <span className={s.groupLabel}>{g.label}</span>
                  <span className={s.groupDesc}>{g.desc}</span>
                </span>
              </label>
            ))}
          </div>
        </Card>
      )}

      {/* 4. 미리보기 */}
      <Card
        title="미리보기"
        description={
          data
            ? `${data.presetLabel} · 파일 ${n(data.files.length)}개 · ${n(data.students)}명`
            : undefined
        }
        actions={
          <Button
            variant="primary"
            icon={FileSpreadsheet}
            disabled={!ready || busy || !data || data.files.length === 0}
            onClick={start}
          >
            {busy ? '만드는 중…' : '내보내기'}
          </Button>
        }
      >
        {!ready ? (
          <Empty title="조건과 양식을 고르면 여기에 미리보기가 나옵니다" />
        ) : !data ? (
          <Empty title="세는 중…" />
        ) : data.files.length === 0 ? (
          <Empty
            icon={FileSpreadsheet}
            title="조건에 맞는 학생이 없습니다"
            description="학년·주소 조건을 넓히거나 학생명단에서 다시 확인해 주세요."
          />
        ) : (
          <>
            {data.warnings.map((w, i) => (
              <WarningLine key={i} w={w} onOpen={setOpenStudent} />
            ))}

            {data.students !== data.matched && (
              <Notice tone="warn">
                조건에 든 {n(data.matched)}명 가운데 <strong>{n(data.students)}명</strong>이
                파일에 들어갑니다.
              </Notice>
            )}

            <div className={s.previewHead}>파일 {n(data.files.length)}개</div>
            <TableWrap>
              <thead>
                <tr>
                  <th>파일 이름</th>
                  <th>구성</th>
                  <th className={tableClass.num}>인원</th>
                </tr>
              </thead>
              <tbody>
                {data.files.map((f) => (
                  <tr key={f.fileName}>
                    <td className="selectable">{f.fileName}.xlsx</td>
                    <td className={tableClass.muted}>
                      {f.sheets.length > 1
                        ? `시트 ${n(f.sheets.length)}장 — ${f.sheets
                            .slice(0, 6)
                            .map((sh) => sh.name)
                            .join(', ')}${f.sheets.length > 6 ? ' …' : ''}`
                        : f.label}
                    </td>
                    <td className={tableClass.num}>{n(f.students)}</td>
                  </tr>
                ))}
              </tbody>
            </TableWrap>

            <div className={s.previewHead}>첫 {n(data.sample.length)}줄</div>
            <TableWrap>
              <thead>
                <tr>
                  {data.headers.map((h) => (
                    <th key={h}>{h}</th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {data.sample.map((row, i) => (
                  <tr key={i}>
                    {row.map((v, j) => (
                      <td key={j}>{v || <span className={tableClass.muted}>—</span>}</td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </TableWrap>
          </>
        )}

        {askOverwrite && (
          <Notice tone="warn">
            {askOverwrite.message}
            <div className={s.confirmRow}>
              <Button variant="danger" onClick={() => doRun(askOverwrite.target, true)}>
                덮어쓰기
              </Button>
              <Button variant="ghost" onClick={() => setAskOverwrite(null)}>
                취소
              </Button>
            </div>
          </Notice>
        )}

        {result && (
          <div className={s.done}>
            <CheckCircle2 size={18} className={s.doneIcon} />
            <div>
              <div className={s.doneTitle}>
                파일 {n(result.files)}개를 만들었습니다 · {n(result.students)}명
              </div>
              <div className={`${s.donePath} selectable`}>{result.folder}</div>
            </div>
            <Button
              icon={FolderOpen}
              onClick={() => exportApi.openFolder(result.folder).catch(setRunError)}
            >
              폴더 열기
            </Button>
          </div>
        )}
      </Card>

      {openStudent !== undefined && (
        <StudentDrawer
          studentId={openStudent}
          schoolYear={schoolYear}
          classSuggestions={Array.from(new Set((classOptions.data ?? []).map((c) => c.className)))}
          onClose={() => setOpenStudent(undefined)}
          onSaved={() => {
            qc.invalidateQueries({ queryKey: ['export-preview'] })
            qc.invalidateQueries({ queryKey: ['students'] })
          }}
        />
      )}

      <p className={s.foot}>
        학생명단에서 조건을 걸고 <button className={s.link} onClick={() => nav('/students')}>
          내보내기
        </button>{' '}
        를 누르면 그 조건이 그대로 넘어옵니다.
      </p>
    </Page>
  )
}

/** 빠지는 학생을 이름으로 보여 주고, 누르면 기존 학생 창을 연다. */
function WarningLine({ w, onOpen }: { w: ExportWarning; onOpen: (id: number) => void }) {
  const shown = w.students.slice(0, MAX_NAMES)
  const rest = w.students.length - shown.length
  return (
    <Notice tone={w.excluded ? 'warn' : 'info'}>
      {w.message}
      <div className={s.names}>
        {shown.map((p) => (
          <button key={p.studentId} className={s.nameChip} onClick={() => onOpen(p.studentId)}>
            {p.label}
          </button>
        ))}
        {rest > 0 && <span className={s.nameMore}>외 {n(rest)}명</span>}
      </div>
    </Notice>
  )
}
