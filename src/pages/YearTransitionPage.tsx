import { useEffect, useMemo, useState } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useNavigate } from 'react-router-dom'
import { open, save } from '@tauri-apps/plugin-dialog'
import {
  ArrowRight,
  CalendarClock,
  CheckCircle2,
  Download,
  FolderOpen,
  RefreshCw,
  Upload,
  UserPlus,
} from 'lucide-react'

import {
  Button,
  Card,
  Empty,
  ErrorNotice,
  Input,
  Notice,
  Page,
  TableWrap,
  tableClass,
} from '@/components/ui'
import { StudentDrawer } from '@/features/student/StudentDrawer'
import { exportApi } from '@/ipc/export'
import { watchJob } from '@/ipc/import'
import { settingsApi } from '@/ipc/settings'
import { studentApi } from '@/ipc/student'
import {
  transitionApi,
  type ApplyResult,
  type Person,
  type Problem,
} from '@/ipc/transition'
import { yearLabel } from '@/lib/schoolYear'
import s from './YearTransitionPage.module.css'

const n = (v: number) => v.toLocaleString('ko-KR')

/** 학생별 표 한 쪽에 몇 줄 */
const PAGE_SIZE = 50
/** 문제 목록에 이름을 몇 명까지 늘어놓을지 */
const MAX_NAMES = 20

/**
 * 학년도 전환.
 *
 * 한 해에 한 번 돌리는 큰 작업이라 **단계로 나눠** 무엇이 생길지 먼저 보여 준다.
 * 지난 학년도는 고치지 않는다 — 2026학년도 3-가람-7 은 그대로 두고 2027학년도
 * 학적을 새로 만든다. 학생 번호가 같으므로 형제 관계와 지난 이력이 이어진다.
 */
export function YearTransitionPage() {
  const nav = useNavigate()
  const qc = useQueryClient()
  const settings = useQuery({ queryKey: ['settings'], queryFn: settingsApi.get })
  const currentYear = settings.data?.currentYear ?? null

  // 원본은 현재 학년도, 대상은 그다음 해다. 화면에서 바꾸지 않고 눈으로 확인만 한다.
  const [fromYear] = useState<number | null>(null)
  const [toYear] = useState<number | null>(null)
  const [assignId, setAssignId] = useState<string | null>(null)
  const [excluded, setExcluded] = useState<number[]>([])
  const [busy, setBusy] = useState<string | null>(null)
  const [error, setError] = useState<unknown>(null)
  const [progress, setProgress] = useState<{ label: string; done: number; total: number } | null>(
    null,
  )
  const [result, setResult] = useState<ApplyResult | null>(null)
  const [openStudent, setOpenStudent] = useState<number | undefined>(undefined)
  const [q, setQ] = useState('')
  const [page, setPage] = useState(0)

  const from = fromYear ?? currentYear
  const to = toYear ?? (from != null ? from + 1 : null)

  const target = useQuery({
    queryKey: ['transition-target', from, to],
    queryFn: () => transitionApi.target(from!, to!),
    enabled: from != null && to != null,
  })

  const classOptions = useQuery({
    queryKey: ['class-options', from],
    queryFn: () => studentApi.classOptions(from!),
    enabled: from != null,
  })

  const plan = useQuery({
    queryKey: ['transition-plan', from, to, assignId, excluded.join(',')],
    queryFn: () =>
      transitionApi.preview({
        fromYear: from!,
        toYear: to!,
        excludeGraduation: excluded,
      }),
    enabled: from != null && to != null && result == null,
    placeholderData: (prev) => prev,
  })

  useEffect(() => setPage(0), [q, assignId, excluded.length])

  const data = plan.data
  const blocking = (data?.problems ?? []).filter((p) => p.blocking)
  const notes = (data?.problems ?? []).filter((p) => !p.blocking)

  const rows = useMemo(() => {
    const all = data?.rows ?? []
    const term = q.trim()
    return term ? all.filter((r) => r.name.includes(term) || r.current.includes(term)) : all
  }, [data, q])
  const lastPage = Math.max(0, Math.ceil(rows.length / PAGE_SIZE) - 1)

  // ---- 진급 배정 양식 ----

  const saveForm = async () => {
    if (from == null) return
    setError(null)
    setBusy('form')
    try {
      const request = { preset: 'PROMOTION' as const, filter: { schoolYear: from } }
      const preview = await exportApi.preview(request)
      if (preview.files.length === 0) {
        setError({ code: 'EMPTY', userMessage: '양식에 넣을 1~5학년 학생이 없습니다.' })
        return
      }
      const path = await save({
        defaultPath: `${preview.defaultFileName ?? '진급배정양식'}.xlsx`,
        filters: [{ name: 'Excel 통합 문서', extensions: ['xlsx'] }],
      })
      if (!path) return // 취소는 오류가 아니다
      const made = await exportApi.run(request, path, true)
      setBusy(null)
      setResultFolder(made.folder)
    } catch (e) {
      setError(e)
    } finally {
      setBusy(null)
    }
  }

  const [formFolder, setResultFolder] = useState<string | null>(null)

  const readAssign = async () => {
    if (from == null) return
    setError(null)
    const path = await open({
      multiple: false,
      filters: [{ name: 'Excel 파일', extensions: ['xlsx', 'xlsm', 'xls'] }],
    })
    if (!path || typeof path !== 'string') return
    setBusy('assign')
    try {
      const info = await transitionApi.readAssign(path, from)
      setAssignId(info.id)
      qc.invalidateQueries({ queryKey: ['transition-plan'] })
    } catch (e) {
      setError(e)
    } finally {
      setBusy(null)
    }
  }

  const dropAssign = async () => {
    await transitionApi.clearAssign()
    setAssignId(null)
    qc.invalidateQueries({ queryKey: ['transition-plan'] })
  }

  // ---- 적용 ----

  const start = async () => {
    if (!data || from == null || to == null) return
    setError(null)
    setBusy('apply')
    setProgress({ label: '백업 만들기', done: 0, total: 1 })
    try {
      const started = await transitionApi.apply({
        fromYear: from,
        toYear: to,
        excludeGraduation: excluded,
        stateKey: data.stateKey,
      })
      const stop = watchJob<ApplyResult>(started.jobId, {
        onProgress: (p) =>
          setProgress({ label: p.stageLabel, done: p.done, total: Math.max(p.total, 1) }),
        onDone: (r) => {
          setResult(r)
          setProgress(null)
          setBusy(null)
          stop()
          qc.invalidateQueries()
        },
        onError: (e) => {
          setError(e)
          setProgress(null)
          setBusy(null)
          stop()
        },
      })
    } catch (e) {
      setError(e)
      setProgress(null)
      setBusy(null)
    }
  }

  const goToNewYear = async () => {
    if (to == null) return
    await settingsApi.setCurrentYear(to)
    qc.invalidateQueries()
    nav('/students')
  }

  if (from == null || to == null) {
    return (
      <Page title="학년도 전환">
        <Empty
          icon={CalendarClock}
          title="먼저 학년도를 만들어 주세요"
          description="설정 화면에서 현재 학년도를 지정하면 다음 학년도를 만들 수 있습니다."
        />
      </Page>
    )
  }

  // ---- 끝난 뒤 ----
  if (result) {
    return (
      <Page title="학년도 전환" year={yearLabel(from)}>
        <Card>
          <div className={s.done}>
            <CheckCircle2 size={22} className={s.doneIcon} />
            <div>
              <div className={s.doneTitle}>
                {result.toYear}학년도 학생명단이 만들어졌습니다
              </div>
              <div className={s.doneText}>
                진급 {n(result.promoted)}명 · 졸업 {n(result.graduated)}명
                {result.skipped > 0 && ` · 학적 없음 ${n(result.skipped)}명`} · 확인 필요{' '}
                {n(result.issues)}건
              </div>
              <div className={s.donePath}>
                전환 전 백업: <span className="selectable">{result.backupPath}</span>
              </div>
            </div>
          </div>

          <Notice tone="info">
            새 1학년 신입생은 아직 들어오지 않았습니다. 가져오기 화면에서 {result.toYear}학년도
            1학년으로 넣으면 주소 판정·중복 확인·형제 후보 찾기가 그대로 이루어집니다.
          </Notice>

          <div className={s.doneActions}>
            <Button variant="primary" icon={ArrowRight} onClick={goToNewYear}>
              {result.toYear}학년도로 이동
            </Button>
            <Button
              icon={UserPlus}
              onClick={() => nav(`/students/import?year=${result.toYear}&grade=1`)}
            >
              신입생 가져오기
            </Button>
            <Button
              variant="ghost"
              icon={FolderOpen}
              onClick={() => exportApi.openFolder(result.backupPath).catch(setError)}
            >
              백업 폴더 열기
            </Button>
          </div>
        </Card>
        <ErrorNotice error={error} />
      </Page>
    )
  }

  const info = target.data

  return (
    <Page
      title="학년도 전환"
      year={yearLabel(from)}
      description="지난 학년도는 그대로 두고 다음 학년도 학적을 새로 만듭니다"
    >
      <ErrorNotice error={error ?? plan.error ?? target.error} />

      {/* 1 — 대상 확인 */}
      <Card title="1. 전환 대상 확인">
        <div className={s.years}>
          <div className={s.yearBox}>
            <span className={s.yearLabel}>원본 학년도</span>
            <strong className={s.yearValue}>{from}학년도</strong>
            <span className={s.yearNote}>재학생 {n(info?.from.active ?? 0)}명</span>
          </div>
          <ArrowRight size={18} className={s.arrow} />
          <div className={s.yearBox}>
            <span className={s.yearLabel}>대상 학년도</span>
            <strong className={s.yearValue}>{to}학년도</strong>
            <span className={s.yearNote}>
              {info?.to.exists ? `학적 ${n(info.to.enrollments)}건` : '아직 만들어지지 않음'}
            </span>
          </div>
        </div>
        <p className={s.hint}>
          진급 대상 {n(info?.promoting ?? 0)}명(1~5학년) · 졸업 대상{' '}
          {n(info?.graduating.length ?? 0)}명(6학년). 지금 전출 상태인 학생은 들어가지 않습니다.
        </p>
        {info?.doneAt && (
          <Notice tone="warn">
            {from}→{to} 전환을 {info.doneAt} 에 이미 실행했습니다. 다시 실행할 수 없습니다.
          </Notice>
        )}
      </Card>

      {/* 2 — 진급 배정 */}
      <Card
        title="2. 새 학급 편성 넣기"
        description="학교가 정한 새 반과 번호를 받아 옵니다. 프로그램이 반을 정하지 않습니다."
      >
        <div className={s.assignRow}>
          <Button icon={Download} disabled={busy != null} onClick={saveForm}>
            진급 배정 양식 받기
          </Button>
          <Button variant="primary" icon={Upload} disabled={busy != null} onClick={readAssign}>
            배정 자료 가져오기
          </Button>
          {assignId && (
            <Button variant="ghost" onClick={dropAssign}>
              읽은 자료 지우기
            </Button>
          )}
        </div>
        {formFolder && (
          <Notice tone="success">
            양식을 만들었습니다. 엑셀에서 <strong>새 반</strong>과 <strong>새 번호</strong>만 채운
            뒤 [배정 자료 가져오기] 로 다시 넣어 주세요.
            <div className={s.confirmRow}>
              <Button
                size="sm"
                icon={FolderOpen}
                onClick={() => exportApi.openFolder(formFolder).catch(setError)}
              >
                폴더 열기
              </Button>
            </div>
          </Notice>
        )}
        {data?.assignFile ? (
          <Notice tone="info">
            <strong>{data.assignFile}</strong> 에서 {n(data.assignRows)}줄을 읽었습니다.
          </Notice>
        ) : (
          <p className={s.hint}>
            새 번호를 비워 두면 그 반 학생을 <strong>이름 가나다순으로 1번부터</strong> 매깁니다.
            일부만 채우면 어느 반인지 알려 드리고 전환을 멈춥니다.
          </p>
        )}
      </Card>

      {/* 3 — 졸업 확인 */}
      <Card
        title="3. 졸업 대상 확인"
        description="기본은 6학년 전체 졸업입니다. 뺀 학생은 졸업도 진급도 하지 않습니다."
      >
        {(info?.graduating.length ?? 0) === 0 ? (
          <p className={s.hint}>6학년 재학생이 없습니다.</p>
        ) : (
          <div className={s.gradList}>
            {info!.graduating.map((p) => (
              <label key={p.studentId} className={s.gradItem}>
                <input
                  type="checkbox"
                  checked={!excluded.includes(p.studentId!)}
                  onChange={(e) =>
                    setExcluded((prev) =>
                      e.target.checked
                        ? prev.filter((id) => id !== p.studentId)
                        : [...prev, p.studentId!],
                    )
                  }
                />
                <span>{p.label}</span>
              </label>
            ))}
          </div>
        )}
        {excluded.length > 0 && (
          <Notice tone="info">
            {n(excluded.length)}명을 졸업에서 뺐습니다. 다음 학년도 학적은 만들어지지 않습니다 —
            전환 뒤에 직접 등록하거나, 배정 자료에 그 학생의 새 학년·반을 적어 주세요.
          </Notice>
        )}
      </Card>

      {/* 4 — 미리보기 */}
      <Card
        title="4. 전환 미리보기"
        description={
          data
            ? `${data.toYear}학년도 생성 예정 — 진급 ${n(data.promoteCount)}명 · 졸업 ${n(
                data.graduateCount,
              )}명`
            : undefined
        }
        actions={
          <Button
            size="sm"
            variant="ghost"
            icon={RefreshCw}
            onClick={() => qc.invalidateQueries({ queryKey: ['transition-plan'] })}
          >
            다시 계산
          </Button>
        }
      >
        {!data ? (
          <Empty title="세는 중…" />
        ) : (
          <>
            {blocking.map((p, i) => (
              <ProblemLine key={i} p={p} onOpen={setOpenStudent} />
            ))}
            {notes.map((p, i) => (
              <ProblemLine key={`n${i}`} p={p} onOpen={setOpenStudent} />
            ))}

            <div className={s.summary}>
              <Stat label="전환 대상" value={data.targetCount} />
              <Stat label="진급" value={data.promoteCount} />
              <Stat label="졸업" value={data.graduateCount} />
              <Stat label="학적 없음" value={data.skipCount} muted />
            </div>

            {data.byGrade.length > 0 && (
              <>
                <div className={s.head}>학년별</div>
                <div className={s.grades}>
                  {data.byGrade.map((g, i) => (
                    <div key={i} className={s.gradeChip}>
                      <span>{g.label}</span>
                      <strong>{n(g.count)}명</strong>
                    </div>
                  ))}
                </div>
              </>
            )}

            {data.classes.length > 0 && (
              <>
                <div className={s.head}>{data.toYear}학년도 예상 학급 현황</div>
                <TableWrap>
                  <thead>
                    <tr>
                      <th>학년</th>
                      <th>반</th>
                      <th className={tableClass.num}>남</th>
                      <th className={tableClass.num}>여</th>
                      <th className={tableClass.num}>미입력</th>
                      <th className={tableClass.num}>합계</th>
                    </tr>
                  </thead>
                  <tbody>
                    {data.classes.map((c) => (
                      <tr key={c.classLabel}>
                        <td>{c.grade}학년</td>
                        <td>{c.className}</td>
                        <td className={tableClass.num}>{n(c.male)}</td>
                        <td className={tableClass.num}>{n(c.female)}</td>
                        <td className={tableClass.num}>{n(c.unknown)}</td>
                        <td className={tableClass.num}>
                          <strong>{n(c.total)}</strong>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </TableWrap>
                <p className={s.hint}>
                  반 편성은 학교가 정합니다. 프로그램은 남녀 인원을 보여 줄 뿐 옮길 학생을 권하지
                  않습니다.
                </p>
              </>
            )}

            <div className={s.head}>학생별 변경</div>
            <Input
              placeholder="이름이나 현재 학반으로 찾기"
              value={q}
              onChange={(e) => setQ(e.target.value)}
              className={s.search}
            />
            <TableWrap>
              <thead>
                <tr>
                  <th>학생</th>
                  <th>현재</th>
                  <th>{data.toYear}학년도</th>
                </tr>
              </thead>
              <tbody>
                {rows.slice(page * PAGE_SIZE, (page + 1) * PAGE_SIZE).map((r) => (
                  <tr key={r.studentId} onClick={() => setOpenStudent(r.studentId)}>
                    <td>{r.name}</td>
                    <td className={tableClass.muted}>{r.current}</td>
                    <td>
                      {r.fate === 'NONE' ? (
                        <span className={s.none}>{r.next}</span>
                      ) : r.fate === 'GRADUATE' ? (
                        <span className={s.grad}>{r.next}</span>
                      ) : (
                        r.next
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </TableWrap>
            <div className={s.pager}>
              <span>
                {n(rows.length)}명 중 {n(Math.min(page * PAGE_SIZE + 1, rows.length))}–
                {n(Math.min((page + 1) * PAGE_SIZE, rows.length))}
              </span>
              <Button size="sm" disabled={page === 0} onClick={() => setPage((p) => p - 1)}>
                이전
              </Button>
              <Button
                size="sm"
                disabled={page >= lastPage}
                onClick={() => setPage((p) => p + 1)}
              >
                다음
              </Button>
            </div>
          </>
        )}
      </Card>

      {/* 5 — 실행 */}
      <Card
        title="5. 다음 학년도 만들기"
        description="적용 직전에 자동으로 백업을 만들고, 한 번에 처리합니다."
      >
        {data?.blocked ? (
          <Notice tone="warn">
            먼저 확인해야 할 것이 {n(blocking.length)}가지 있습니다. 위에서 확인한 뒤 배정 자료를
            고쳐 다시 넣어 주세요.
          </Notice>
        ) : (
          <p className={s.hint}>
            {n(data?.promoteCount ?? 0)}명의 {to}학년도 학적을 만들고 {n(data?.graduateCount ?? 0)}
            명을 졸업 처리합니다. {from}학년도 기록은 그대로 남습니다.
          </p>
        )}

        {progress && (
          <div className={s.progress}>
            <div className={s.progressLabel}>
              {progress.label} {progress.total > 1 && `${n(progress.done)}/${n(progress.total)}`}
            </div>
            <div className={s.bar}>
              <div
                className={s.barFill}
                style={{ width: `${Math.round((progress.done / progress.total) * 100)}%` }}
              />
            </div>
          </div>
        )}

        <Button
          variant="primary"
          icon={CalendarClock}
          disabled={!data || data.blocked || busy != null}
          onClick={start}
        >
          {busy === 'apply' ? '전환하는 중…' : `${to}학년도 만들기`}
        </Button>
      </Card>

      {openStudent !== undefined && (
        <StudentDrawer
          studentId={openStudent}
          schoolYear={from}
          classSuggestions={Array.from(new Set((classOptions.data ?? []).map((c) => c.className)))}
          onClose={() => setOpenStudent(undefined)}
          onSaved={() => qc.invalidateQueries({ queryKey: ['transition-plan'] })}
        />
      )}
    </Page>
  )
}

function Stat({ label, value, muted }: { label: string; value: number; muted?: boolean }) {
  return (
    <div className={muted ? `${s.stat} ${s.statMuted}` : s.stat}>
      <span className={s.statLabel}>{label}</span>
      <strong className={s.statValue}>{n(value)}</strong>
    </div>
  )
}

/** 무엇이 문제이고 누구인지. 누르면 기존 학생 창이 열린다. */
function ProblemLine({ p, onOpen }: { p: Problem; onOpen: (id: number) => void }) {
  const shown = p.students.slice(0, MAX_NAMES)
  const rest = p.students.length - shown.length
  return (
    <Notice tone={p.blocking ? 'warn' : 'info'}>
      <strong>{p.title}</strong> {p.students.length > 0 && `${n(p.students.length)}명`} — {p.message}
      {shown.length > 0 && (
        <div className={s.names}>
          {shown.map((x: Person, i) => (
            <button
              key={`${x.studentId}-${i}`}
              className={s.nameChip}
              disabled={x.studentId == null}
              onClick={() => x.studentId != null && onOpen(x.studentId)}
              title={x.detail ?? undefined}
            >
              {x.label}
              {x.detail && <span className={s.detail}>{x.detail}</span>}
            </button>
          ))}
          {rest > 0 && <span className={s.nameMore}>외 {n(rest)}명</span>}
        </div>
      )}
    </Notice>
  )
}
