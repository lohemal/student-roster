import { useEffect, useMemo, useRef, useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useNavigate } from 'react-router-dom'
import { open } from '@tauri-apps/plugin-dialog'
import {
  AlertTriangle,
  ArrowLeft,
  CheckCircle2,
  ChevronRight,
  FileSpreadsheet,
  FolderOpen,
} from 'lucide-react'

import {
  Badge,
  Button,
  Card,
  Empty,
  ErrorNotice,
  Field,
  Notice,
  Page,
  Select,
  TableWrap,
  Tabs,
  tableClass,
  type TabDef,
} from '@/components/ui'
import {
  importApi,
  watchJob,
  type Action,
  type AnalyzeResponse,
  type ApplyResult,
  type FileInfo,
  type JobProgress,
  type Mapping,
  type RowPlan,
  type SheetPreview,
} from '@/ipc/import'
import { settingsApi } from '@/ipc/settings'
import { yearLabel } from '@/lib/schoolYear'
import s from './ImportPage.module.css'

type Step = 'file' | 'map' | 'review' | 'run' | 'done'

const STEPS: { key: Step; label: string }[] = [
  { key: 'file', label: '파일 선택' },
  { key: 'map', label: '열 연결' },
  { key: 'review', label: '분석 결과' },
  { key: 'run', label: '가져오기' },
]

function stepIndex(step: Step): number {
  if (step === 'done') return STEPS.length
  return STEPS.findIndex((x) => x.key === step)
}

function Stepper({ step }: { step: Step }) {
  const now = stepIndex(step)
  return (
    <div className={s.steps}>
      {STEPS.map((x, i) => (
        <span key={x.key} style={{ display: 'contents' }}>
          {i > 0 && <ChevronRight size={14} className={s.stepArrow} />}
          <span
            className={
              i < now ? `${s.step} ${s.stepDone}` : i === now ? `${s.step} ${s.stepNow}` : s.step
            }
          >
            <span className={s.stepNum}>{i + 1}</span>
            {x.label}
          </span>
        </span>
      ))}
    </div>
  )
}

const ACTION_TONE: Record<Action, 'info' | 'success' | 'neutral' | 'warn' | 'error'> = {
  add: 'info',
  update: 'success',
  unchanged: 'neutral',
  ambiguous: 'warn',
  blocked: 'error',
}

type ReviewTab = 'all' | Action | 'warned'

export function ImportPage() {
  const nav = useNavigate()
  const qc = useQueryClient()
  const settings = useQuery({ queryKey: ['settings'], queryFn: settingsApi.get })
  const schoolYear = settings.data?.currentYear ?? null

  const [step, setStep] = useState<Step>('file')
  const [path, setPath] = useState<string | null>(null)
  const [file, setFile] = useState<FileInfo | null>(null)
  const [sheet, setSheet] = useState<string | null>(null)
  const [preview, setPreview] = useState<SheetPreview | null>(null)
  const [mapping, setMapping] = useState<Mapping>({})
  const [defaultGrade, setDefaultGrade] = useState<number>(1)
  const [analysis, setAnalysis] = useState<AnalyzeResponse | null>(null)
  const [tab, setTab] = useState<ReviewTab>('all')
  const [doAdd, setDoAdd] = useState(true)
  const [doUpdate, setDoUpdate] = useState(true)
  const [progress, setProgress] = useState<JobProgress | null>(null)
  const [result, setResult] = useState<ApplyResult | null>(null)
  const [runError, setRunError] = useState<unknown>(null)
  const stopWatch = useRef<(() => void) | null>(null)

  useEffect(() => () => stopWatch.current?.(), [])

  // ---- 1단계: 파일 ----
  const pickFile = useMutation({
    mutationFn: async () => {
      const picked = await open({
        multiple: false,
        directory: false,
        filters: [{ name: '엑셀 파일', extensions: ['xlsx', 'xlsm', 'xls'] }],
      })
      if (typeof picked !== 'string') return null
      const info = await importApi.inspect(picked)
      return { picked, info }
    },
    onSuccess: (v) => {
      if (!v) return
      setPath(v.picked)
      setFile(v.info)
      setAnalysis(null)
      setPreview(null)
      // 표가 있는 시트가 하나뿐이면 바로 고른다
      const usable = v.info.sheets.filter((x) => x.rows > 0)
      setSheet(usable.length === 1 ? usable[0].name : null)
    },
  })

  // ---- 2단계: 열 연결 ----
  const loadPreview = useMutation({
    mutationFn: (args: { sheet: string; headerRow?: number }) =>
      importApi.preview(path!, args.sheet, args.headerRow),
    onSuccess: (p) => {
      setPreview(p)
      setMapping(p.mapping)
      setStep('map')
    },
  })

  // ---- 3단계: 분석 ----
  const analyze = useMutation({
    mutationFn: () =>
      importApi.analyze({
        path: path!,
        sheet: sheet!,
        headerRow: preview!.headerRow,
        mapping,
        schoolYear: schoolYear!,
        defaultGrade: mapping.grade === undefined ? defaultGrade : null,
      }),
    onSuccess: (r) => {
      setAnalysis(r)
      setTab('all')
      setStep('review')
    },
  })

  // ---- 4단계: 적용 ----
  const apply = useMutation({
    mutationFn: async () => {
      setRunError(null)
      setProgress(null)
      setResult(null)
      const started = await importApi.apply(analysis!.analysisId, {
        add: doAdd,
        update: doUpdate,
        skipRows: [],
      })
      setStep('run')
      stopWatch.current?.()
      stopWatch.current = watchJob<ApplyResult>(started.jobId, {
        onProgress: setProgress,
        onDone: (r) => {
          setResult(r)
          setStep('done')
          qc.invalidateQueries()
        },
        onError: (e) => {
          setRunError(e)
          setStep('review')
        },
      })
      return started
    },
    onError: (e) => setRunError(e),
  })

  const columns = preview?.headers ?? []

  // 한 엑셀 열이 두 항목에 겹쳐 붙었는지
  const duplicated = useMemo(() => {
    const seen = new Map<number, number>()
    Object.values(mapping).forEach((col) => {
      if (col !== undefined) seen.set(col, (seen.get(col) ?? 0) + 1)
    })
    return new Set([...seen.entries()].filter(([, n]) => n > 1).map(([col]) => col))
  }, [mapping])

  const nameMapped = mapping.name !== undefined
  const rows = analysis?.rows ?? []
  const shown = useMemo(() => {
    if (!analysis) return []
    if (tab === 'all') return rows
    if (tab === 'warned') return rows.filter((r) => r.warnings.length > 0)
    return rows.filter((r) => r.action === tab)
  }, [analysis, rows, tab])

  const sum = analysis?.summary
  const tabs: TabDef<ReviewTab>[] = sum
    ? [
        { key: 'all', label: '전체', count: sum.total },
        { key: 'add', label: '신규', count: sum.add },
        { key: 'update', label: '갱신', count: sum.update },
        { key: 'unchanged', label: '변경 없음', count: sum.unchanged },
        { key: 'ambiguous', label: '중복 의심', count: sum.ambiguous },
        { key: 'blocked', label: '가져올 수 없음', count: sum.blocked },
        { key: 'warned', label: '확인 필요 예상', count: sum.warned },
      ]
    : []

  const willApply = sum ? (doAdd ? sum.add : 0) + (doUpdate ? sum.update : 0) : 0
  const running = step === 'run'

  if (schoolYear == null) {
    return (
      <Page title="학생명단 가져오기">
        <Empty
          icon={FileSpreadsheet}
          title="먼저 학년도를 만들어 주세요"
          description="설정 화면에서 현재 학년도를 지정하면 명단을 가져올 수 있습니다."
        />
      </Page>
    )
  }

  return (
    <Page
      title="학생명단 가져오기"
      year={yearLabel(schoolYear)}
      actions={
        !running && (
          <Button variant="outline" icon={ArrowLeft} onClick={() => nav('/students')}>
            학생명단으로
          </Button>
        )
      }
    >
      <Stepper step={step} />
      <ErrorNotice error={pickFile.error ?? loadPreview.error ?? analyze.error ?? runError} />

      {/* ---------- 1단계 ---------- */}
      {step === 'file' && (
        <Card
          title="엑셀 파일 고르기"
          description="학생명단이 들어 있는 .xlsx 파일을 고르세요. 고르기만 해서는 자료가 바뀌지 않습니다."
        >
          <div className={s.stack}>
            {!file ? (
              <Empty
                icon={FileSpreadsheet}
                title="가져올 파일을 골라 주세요"
                description="파일을 읽어 시트와 열을 먼저 보여 드립니다. 확인한 뒤에 가져옵니다."
                action={
                  <Button
                    variant="primary"
                    icon={FolderOpen}
                    onClick={() => pickFile.mutate()}
                    disabled={pickFile.isPending}
                  >
                    파일 고르기
                  </Button>
                }
              />
            ) : (
              <>
                <div className={s.fileBox}>
                  <FileSpreadsheet size={22} color="var(--success-text)" />
                  <div style={{ flex: 1, minWidth: 0 }}>
                    <div className={s.fileName}>{file.fileName}</div>
                    <div className={s.filePath}>{path}</div>
                  </div>
                  <Button variant="outline" onClick={() => pickFile.mutate()}>
                    다른 파일
                  </Button>
                </div>

                <div>
                  <div className={s.mapLabel} style={{ width: 'auto', marginBottom: 8 }}>
                    가져올 시트
                  </div>
                  <div className={s.sheetList}>
                    {file.sheets.map((x) => (
                      <button
                        key={x.name}
                        type="button"
                        className={x.name === sheet ? `${s.sheetBtn} ${s.sheetOn}` : s.sheetBtn}
                        onClick={() => setSheet(x.name)}
                        disabled={x.rows === 0}
                      >
                        <span className={s.sheetName}>{x.name}</span>
                        <span className={s.sheetMeta}>
                          {x.rows === 0 ? '표가 없습니다' : `${x.rows}줄 · ${x.cols}열`}
                        </span>
                      </button>
                    ))}
                  </div>
                </div>

                <div className={s.foot}>
                  <Button
                    variant="primary"
                    onClick={() => loadPreview.mutate({ sheet: sheet! })}
                    disabled={!sheet || loadPreview.isPending}
                  >
                    다음 — 열 연결
                  </Button>
                </div>
              </>
            )}
          </div>
        </Card>
      )}

      {/* ---------- 2단계 ---------- */}
      {step === 'map' && preview && (
        <>
          <Card
            title="열 연결"
            description="자동으로 찾아 둔 것이 맞는지 확인하고, 다르면 바꿔 주세요."
            actions={
              <div className={s.row}>
                <span className={s.sheetMeta}>
                  헤더 줄 {preview.headerRow + 1} · 자료 {preview.totalRows.toLocaleString()}줄
                </span>
                <Select
                  value={preview.headerRow}
                  onChange={(e) =>
                    loadPreview.mutate({ sheet: sheet!, headerRow: Number(e.target.value) })
                  }
                >
                  {Array.from({ length: Math.min(10, preview.headerRow + 6) }, (_, i) => i).map(
                    (i) => (
                      <option key={i} value={i}>
                        헤더는 {i + 1}번째 줄
                      </option>
                    ),
                  )}
                </Select>
              </div>
            }
          >
            <div className={s.stack}>
              {!nameMapped && (
                <Notice tone="error">
                  이름 열을 지정해 주세요. 이름이 없으면 학생을 가져올 수 없습니다.
                </Notice>
              )}
              {duplicated.size > 0 && (
                <Notice tone="warn">
                  한 엑셀 열을 여러 항목에 연결했습니다. 그대로 두면 같은 값이 여러 항목에
                  들어갑니다. 아래 노란 칸을 확인해 주세요.
                </Notice>
              )}
              {mapping.grade === undefined && (
                <div className={s.row}>
                  <Notice tone="info">
                    학년 열이 없습니다. 이 시트의 학생을 모두 같은 학년으로 넣으려면 아래에서
                    골라 주세요.
                  </Notice>
                  <Field label="이 시트의 학년">
                    <Select
                      value={defaultGrade}
                      onChange={(e) => setDefaultGrade(Number(e.target.value))}
                    >
                      {[1, 2, 3, 4, 5, 6].map((g) => (
                        <option key={g} value={g}>
                          {g}학년
                        </option>
                      ))}
                    </Select>
                  </Field>
                </div>
              )}

              <div className={s.mapGrid}>
                {preview.fields.map((f) => {
                  const col = mapping[f.key]
                  const dup = col !== undefined && duplicated.has(col)
                  const missing = f.required && col === undefined
                  return (
                    <div
                      key={f.key}
                      className={
                        missing
                          ? `${s.mapItem} ${s.mapItemMissing}`
                          : dup
                            ? `${s.mapItem} ${s.mapItemDup}`
                            : s.mapItem
                      }
                    >
                      <span className={s.mapLabel}>
                        {f.label}
                        {f.required && <span className={s.mapRequired}>*</span>}
                      </span>
                      <Select
                        className={s.mapSelect}
                        value={col ?? ''}
                        onChange={(e) => {
                          const v = e.target.value
                          setMapping((m) => {
                            const next = { ...m }
                            if (v === '') delete next[f.key]
                            else next[f.key] = Number(v)
                            return next
                          })
                        }}
                      >
                        <option value="">사용 안 함</option>
                        {columns.map((h, i) => (
                          <option key={i} value={i}>
                            {h || `(${i + 1}번째 열)`}
                          </option>
                        ))}
                      </Select>
                    </div>
                  )
                })}
              </div>

              <div>
                <div className={s.mapLabel} style={{ width: 'auto', marginBottom: 8 }}>
                  파일 미리보기
                </div>
                <div className={s.preview}>
                  <TableWrap>
                    <thead>
                      <tr>
                        {columns.map((h, i) => (
                          <th key={i}>{h || `(${i + 1})`}</th>
                        ))}
                      </tr>
                    </thead>
                    <tbody>
                      {preview.sampleRows.map((r, i) => (
                        <tr key={i} style={{ cursor: 'default' }}>
                          {columns.map((_, c) => (
                            <td key={c} className={s.previewCell}>
                              {r[c] ?? ''}
                            </td>
                          ))}
                        </tr>
                      ))}
                    </tbody>
                  </TableWrap>
                </div>
              </div>

              <div className={s.foot}>
                <span className={s.footSpacer} />
                <Button variant="outline" onClick={() => setStep('file')}>
                  뒤로
                </Button>
                <Button
                  variant="primary"
                  onClick={() => analyze.mutate()}
                  disabled={!nameMapped || analyze.isPending}
                >
                  {analyze.isPending ? '분석 중…' : '다음 — 분석하기'}
                </Button>
              </div>
            </div>
          </Card>
        </>
      )}

      {/* ---------- 3단계 ---------- */}
      {step === 'review' && analysis && sum && (
        <>
          <Card
            title={`${analysis.fileName} · ${analysis.sheetName}`}
            description={`${analysis.schoolYear}학년도로 가져옵니다. 아직 아무것도 저장하지 않았습니다.`}
          >
            <div className={s.summary}>
              <div className={`${s.tile} ${s.tileAdd}`}>
                <div className={s.tileNum}>{sum.add.toLocaleString()}</div>
                <div className={s.tileLabel}>신규 추가</div>
              </div>
              <div className={`${s.tile} ${s.tileUpdate}`}>
                <div className={s.tileNum}>{sum.update.toLocaleString()}</div>
                <div className={s.tileLabel}>기존 학생 갱신</div>
              </div>
              <div className={s.tile}>
                <div className={s.tileNum}>{sum.unchanged.toLocaleString()}</div>
                <div className={s.tileLabel}>변경 없음</div>
              </div>
              <div className={`${s.tile} ${s.tileWarn}`}>
                <div className={s.tileNum}>{sum.ambiguous.toLocaleString()}</div>
                <div className={s.tileLabel}>중복 의심</div>
              </div>
              <div className={`${s.tile} ${s.tileError}`}>
                <div className={s.tileNum}>{sum.blocked.toLocaleString()}</div>
                <div className={s.tileLabel}>가져올 수 없음</div>
              </div>
              <div className={`${s.tile} ${s.tileWarn}`}>
                <div className={s.tileNum}>{sum.warned.toLocaleString()}</div>
                <div className={s.tileLabel}>확인 필요 예상</div>
              </div>
            </div>
          </Card>

          {(sum.ambiguous > 0 || sum.blocked > 0) && (
            <Notice tone="warn">
              중복 의심 {sum.ambiguous}줄과 가져올 수 없는 {sum.blocked}줄은 저장하지 않습니다.
              같은 학생인지 확실하지 않은 것을 프로그램이 함부로 고치지 않기 위해서입니다. 목록에서
              확인한 뒤 학생명단에서 직접 처리해 주세요.
            </Notice>
          )}

          <Card>
            <Tabs tabs={tabs} active={tab} onChange={setTab} />
            <div style={{ marginTop: 12 }}>
              {shown.length === 0 ? (
                <Empty title="해당하는 줄이 없습니다" />
              ) : (
                <TableWrap>
                  <thead>
                    <tr>
                      <th style={{ width: 56 }} className={tableClass.num}>
                        줄
                      </th>
                      <th style={{ width: 110 }}>처리</th>
                      <th style={{ width: 110 }}>이름</th>
                      <th style={{ width: 110 }}>학년-반-번호</th>
                      <th style={{ width: 92 }}>생년월일</th>
                      <th>내용</th>
                    </tr>
                  </thead>
                  <tbody>
                    {shown.slice(0, 300).map((r) => (
                      <PlanRow key={r.excelRow} row={r} />
                    ))}
                  </tbody>
                </TableWrap>
              )}
              {shown.length > 300 && (
                <div className={s.sheetMeta} style={{ marginTop: 8 }}>
                  {shown.length.toLocaleString()}줄 가운데 앞 300줄만 보여 줍니다. 가져오기는
                  전체에 적용됩니다.
                </div>
              )}
            </div>
          </Card>

          <Card title="무엇을 적용할까요">
            <div className={s.stack}>
              <label className={s.row} style={{ cursor: 'pointer' }}>
                <input type="checkbox" checked={doAdd} onChange={(e) => setDoAdd(e.target.checked)} />
                <span>
                  <b>신규 추가</b> {sum.add.toLocaleString()}명 — 새 학생으로 등록합니다.
                </span>
              </label>
              <label className={s.row} style={{ cursor: 'pointer' }}>
                <input
                  type="checkbox"
                  checked={doUpdate}
                  onChange={(e) => setDoUpdate(e.target.checked)}
                />
                <span>
                  <b>기존 학생 갱신</b> {sum.update.toLocaleString()}명 — 위 [갱신] 탭에 적힌
                  내용대로 고칩니다. 엑셀 빈칸은 기존 값을 지우지 않습니다.
                </span>
              </label>
              <div className={s.foot}>
                <span className={s.footSpacer} />
                <Button variant="outline" onClick={() => setStep('map')}>
                  뒤로
                </Button>
                <Button
                  variant="primary"
                  onClick={() => apply.mutate()}
                  disabled={apply.isPending || willApply === 0}
                >
                  {willApply.toLocaleString()}명 가져오기
                </Button>
              </div>
            </div>
          </Card>
        </>
      )}

      {/* ---------- 4단계 ---------- */}
      {step === 'run' && (
        <Card>
          <div className={s.progressWrap}>
            <div className={s.progressStage}>학생명단 가져오는 중</div>
            <div className={s.percent}>
              {progress && progress.total > 0
                ? Math.round((progress.done / progress.total) * 100)
                : 0}
              %
            </div>
            <div className={s.bar}>
              <div
                className={s.barFill}
                style={{
                  width: `${progress && progress.total > 0 ? Math.round((progress.done / progress.total) * 100) : 0}%`,
                }}
              />
            </div>
            <div className={s.progressCount}>
              {progress
                ? `${progress.done.toLocaleString()} / ${progress.total.toLocaleString()}`
                : '준비 중…'}
            </div>
            <div className={s.progressStage}>
              현재 작업: {progress?.stageLabel ?? '시작하는 중'}
            </div>
            <Notice tone="info">
              끝날 때까지 창을 닫지 마세요. 중간에 문제가 생기면 아무것도 저장되지 않습니다.
            </Notice>
          </div>
        </Card>
      )}

      {/* ---------- 완료 ---------- */}
      {step === 'done' && result && (
        <Card>
          <div className={s.progressWrap}>
            <div className={s.doneMark}>
              <CheckCircle2 size={30} />
            </div>
            <h2 style={{ fontSize: 'var(--fs-xl)' }}>학생명단 가져오기 완료</h2>
            <div className={s.progressCount}>
              모두 {result.total.toLocaleString()}줄을 살펴봤습니다.
            </div>

            <div className={s.resultList}>
              <div className={s.resultRow}>
                <span>신규 등록</span>
                <span className={s.resultNum}>{result.added.toLocaleString()}명</span>
              </div>
              <div className={s.resultRow}>
                <span>기존 학생 갱신</span>
                <span className={s.resultNum}>{result.updated.toLocaleString()}명</span>
              </div>
              <div className={s.resultRow}>
                <span>변경 없음</span>
                <span className={s.resultNum}>{result.unchanged.toLocaleString()}명</span>
              </div>
              <div className={s.resultRow}>
                <span>적용 제외</span>
                <span className={s.resultNum}>{result.skipped.toLocaleString()}줄</span>
              </div>
              {result.siblingCandidates > 0 && (
                <div className={s.resultRow}>
                  <span>새 형제 후보</span>
                  <span className={s.resultNum}>
                    {result.siblingCandidates.toLocaleString()}쌍
                  </span>
                </div>
              )}
              <div className={s.resultRow}>
                <span>확인 필요</span>
                <span className={s.resultNum}>{result.issueCount.toLocaleString()}건</span>
              </div>
            </div>

            {result.siblingCandidates > 0 && (
              <div className={s.resultHint}>
                보호자 정보가 비슷한 학생을 찾았습니다. 학생 상세의 [형제] 탭에서 형제가 맞는지
                확인해 주세요.
              </div>
            )}

            <div className={s.row} style={{ marginTop: 16 }}>
              <Button variant="primary" onClick={() => nav('/students')}>
                학생명단 보기
              </Button>
              <Button
                variant="outline"
                onClick={() => {
                  setStep('file')
                  setFile(null)
                  setPath(null)
                  setSheet(null)
                  setPreview(null)
                  setAnalysis(null)
                  setResult(null)
                }}
              >
                다른 파일 가져오기
              </Button>
            </div>
          </div>
        </Card>
      )}
    </Page>
  )
}

function PlanRow({ row }: { row: RowPlan }) {
  return (
    <tr style={{ cursor: 'default' }}>
      <td className={tableClass.num}>{row.excelRow}</td>
      <td>
        <Badge tone={ACTION_TONE[row.action]}>{row.actionLabel}</Badge>
      </td>
      <td style={{ fontWeight: 600 }}>{row.name || <span className={tableClass.muted}>없음</span>}</td>
      <td>{row.whereAt}</td>
      <td>{row.birth}</td>
      <td className={s.wideCell}>
        {row.changes.length > 0 && (
          <div className={s.changeList}>
            {row.changes.map((c, i) => (
              <div key={i} className={s.change}>
                <span className={s.changeField}>{c.field}</span>
                {c.before && <span className={s.changeBefore}>{c.before}</span>}
                <span>→</span>
                <span className={s.changeAfter}>{c.after}</span>
              </div>
            ))}
          </div>
        )}
        {row.reason && <div className={s.reason}>{row.reason}</div>}
        {row.candidates.length > 0 && (
          <div className={s.candidates}>
            헷갈리는 학생:{' '}
            {row.candidates.map((c) => `${c.name} (${c.birth}, ${c.whereAt})`).join(' · ')}
          </div>
        )}
        {row.warnings.length > 0 && (
          <div className={s.candidates}>
            <AlertTriangle size={11} style={{ verticalAlign: -1, marginRight: 3 }} />
            {row.warnings.map((w) => w.message).join(' ')}
          </div>
        )}
        {row.action === 'unchanged' && row.changes.length === 0 && !row.reason && (
          <span className={tableClass.muted}>바뀐 내용이 없습니다</span>
        )}
      </td>
    </tr>
  )
}
