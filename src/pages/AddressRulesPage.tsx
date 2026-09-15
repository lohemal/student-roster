import { useEffect, useRef, useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { Check, MapPinned, Pencil, Plus, RefreshCw, Trash2, X } from 'lucide-react'

import {
  Badge,
  Button,
  Card,
  Empty,
  ErrorNotice,
  Field,
  Input,
  Notice,
  Page,
  Select,
  TableWrap,
  tableClass,
} from '@/components/ui'
import {
  addressApi,
  RULE_KIND_HINT,
  RULE_KIND_LABEL,
  type Matching,
  type ReapplyResult,
  type RuleKind,
  type RuleRow,
} from '@/ipc/address'
import { watchJob, type JobProgress } from '@/ipc/import'
import { settingsApi } from '@/ipc/settings'
import { yearLabel } from '@/lib/schoolYear'
import s from './AddressRulesPage.module.css'

const KINDS: RuleKind[] = ['ROAD', 'COMPLEX', 'CONTAINS']

export function AddressRulesPage() {
  const qc = useQueryClient()
  const settings = useQuery({ queryKey: ['settings'], queryFn: settingsApi.get })
  const schoolYear = settings.data?.currentYear ?? null

  const categories = useQuery({
    queryKey: ['address-categories', schoolYear],
    queryFn: () => addressApi.categories(schoolYear!),
    enabled: schoolYear != null,
  })
  const rules = useQuery({
    queryKey: ['address-rules', schoolYear],
    queryFn: () => addressApi.rules(schoolYear!),
    enabled: schoolYear != null,
  })

  const refresh = () => {
    qc.invalidateQueries({ queryKey: ['address-categories'] })
    qc.invalidateQueries({ queryKey: ['address-rules'] })
    qc.invalidateQueries({ queryKey: ['students'] })
    qc.invalidateQueries({ queryKey: ['issue-summary'] })
    qc.invalidateQueries({ queryKey: ['issues'] })
    qc.invalidateQueries({ queryKey: ['student'] })
  }

  // ---- 분류 ----
  const [newCat, setNewCat] = useState('')
  const [editCat, setEditCat] = useState<{ id: number; name: string } | null>(null)

  const createCat = useMutation({
    mutationFn: () => addressApi.createCategory(newCat),
    onSuccess: () => {
      setNewCat('')
      refresh()
    },
  })
  const renameCat = useMutation({
    mutationFn: () => addressApi.renameCategory(editCat!.id, editCat!.name),
    onSuccess: () => {
      setEditCat(null)
      refresh()
    },
  })
  const deleteCat = useMutation({
    mutationFn: (id: number) => addressApi.deleteCategory(id),
    onSuccess: refresh,
  })

  // ---- 규칙 ----
  const [kind, setKind] = useState<RuleKind>('ROAD')
  const [pattern, setPattern] = useState('')
  const [catId, setCatId] = useState<number | null>(null)
  const [editRule, setEditRule] = useState<RuleRow | null>(null)
  const [madeRule, setMadeRule] = useState<{ ruleId: number; matching: Matching } | null>(null)

  useEffect(() => {
    if (catId == null && categories.data?.length) setCatId(categories.data[0].id)
  }, [categories.data, catId])

  const createRule = useMutation({
    mutationFn: () =>
      addressApi.createRule({ kind, pattern, categoryId: catId! }, schoolYear!),
    onSuccess: (r) => {
      setPattern('')
      setMadeRule(r)
      refresh()
    },
  })
  const applyRule = useMutation({
    mutationFn: (ruleId: number) => addressApi.applyRule(ruleId, schoolYear!),
    onSuccess: () => {
      setMadeRule(null)
      refresh()
    },
  })
  const saveRule = useMutation({
    mutationFn: () =>
      addressApi.updateRule({
        id: editRule!.id,
        kind: editRule!.kind,
        pattern: editRule!.pattern,
        categoryId: editRule!.categoryId,
        isActive: editRule!.isActive,
      }),
    onSuccess: () => {
      setEditRule(null)
      refresh()
    },
  })
  const toggleRule = useMutation({
    mutationFn: (r: RuleRow) =>
      addressApi.updateRule({
        id: r.id,
        kind: r.kind,
        pattern: r.pattern,
        categoryId: r.categoryId,
        isActive: !r.isActive,
      }),
    onSuccess: refresh,
  })
  const deleteRule = useMutation({
    mutationFn: (id: number) => addressApi.deleteRule(id),
    onSuccess: refresh,
  })

  // ---- 전체 재적용 ----
  const [progress, setProgress] = useState<JobProgress | null>(null)
  const [reapplyResult, setReapplyResult] = useState<ReapplyResult | null>(null)
  const [reapplyError, setReapplyError] = useState<unknown>(null)
  const [running, setRunning] = useState(false)
  const stopWatch = useRef<(() => void) | null>(null)
  useEffect(() => () => stopWatch.current?.(), [])

  const reapply = useMutation({
    mutationFn: async () => {
      setReapplyError(null)
      setReapplyResult(null)
      setProgress(null)
      setRunning(true)
      const started = await addressApi.reapply(schoolYear!)
      stopWatch.current?.()
      stopWatch.current = watchJob<ReapplyResult>(started.jobId, {
        onProgress: setProgress,
        onDone: (r) => {
          setReapplyResult(r)
          setRunning(false)
          refresh()
        },
        onError: (e) => {
          setReapplyError(e)
          setRunning(false)
        },
      })
    },
    onError: (e) => {
      setReapplyError(e)
      setRunning(false)
    },
  })

  if (schoolYear == null) {
    return (
      <Page title="주소 규칙">
        <Empty
          icon={MapPinned}
          title="먼저 학년도를 만들어 주세요"
          description="설정 화면에서 현재 학년도를 지정하면 주소 규칙을 쓸 수 있습니다."
        />
      </Page>
    )
  }

  const cats = categories.data ?? []
  const pct = progress && progress.total > 0 ? Math.round((progress.done / progress.total) * 100) : 0

  return (
    <Page
      title="주소 규칙"
      year={yearLabel(schoolYear)}
      actions={
        <Button
          variant="outline"
          icon={RefreshCw}
          onClick={() => reapply.mutate()}
          disabled={running}
        >
          {running ? '적용 중…' : '주소 규칙 다시 적용'}
        </Button>
      }
    >
      <ErrorNotice error={categories.error ?? rules.error ?? reapplyError} />

      {running && (
        <Card>
          <div className={s.stack}>
            <div className={s.row}>
              <span style={{ fontWeight: 700 }}>주소 규칙 적용 중</span>
              <span className={s.progressText}>{pct}%</span>
              <span className={s.progressText}>
                {progress
                  ? `${progress.done.toLocaleString()} / ${progress.total.toLocaleString()}명`
                  : ''}
              </span>
            </div>
            <div className={s.bar}>
              <div className={s.barFill} style={{ width: `${pct}%` }} />
            </div>
            <span className={s.progressText}>
              현재 작업: {progress?.stageLabel ?? '시작하는 중'}
            </span>
          </div>
        </Card>
      )}

      {reapplyResult && !running && (
        <Card title="다시 적용했습니다" description={`재학생 ${reapplyResult.total.toLocaleString()}명을 살펴봤습니다.`}>
          <div className={s.resultGrid}>
            <div className={`${s.tile} ${s.tileOk}`}>
              <div className={s.tileNum}>{reapplyResult.byRule.toLocaleString()}</div>
              <div className={s.tileLabel}>규칙으로 분류</div>
            </div>
            <div className={`${s.tile} ${s.tileOk}`}>
              <div className={s.tileNum}>{reapplyResult.byAuto.toLocaleString()}</div>
              <div className={s.tileLabel}>단지명으로 자동</div>
            </div>
            <div className={s.tile}>
              <div className={s.tileNum}>{reapplyResult.keptManual.toLocaleString()}</div>
              <div className={s.tileLabel}>직접 지정 유지</div>
            </div>
            <div className={`${s.tile} ${s.tileWarn}`}>
              <div className={s.tileNum}>{reapplyResult.unclassified.toLocaleString()}</div>
              <div className={s.tileLabel}>미분류</div>
            </div>
            <div className={`${s.tile} ${s.tileError}`}>
              <div className={s.tileNum}>{reapplyResult.conflict.toLocaleString()}</div>
              <div className={s.tileLabel}>규칙 충돌</div>
            </div>
            <div className={s.tile}>
              <div className={s.tileNum}>{reapplyResult.noAddress.toLocaleString()}</div>
              <div className={s.tileLabel}>주소 없음</div>
            </div>
          </div>
        </Card>
      )}

      <div className={s.grid}>
        {/* ---------- 분류 ---------- */}
        <Card
          title="주소 분류"
          description="학교에서 쓰는 분류를 만듭니다. 통계는 이 이름으로 묶입니다."
        >
          <div className={s.stack}>
            <ErrorNotice error={createCat.error ?? renameCat.error ?? deleteCat.error} />

            <div className={s.catList}>
              {cats.map((c) =>
                editCat?.id === c.id ? (
                  <div key={c.id} className={s.catRow}>
                    <Input
                      value={editCat.name}
                      onChange={(e) => setEditCat({ ...editCat, name: e.target.value })}
                      onKeyDown={(e) => e.key === 'Enter' && renameCat.mutate()}
                      autoFocus
                    />
                    <button
                      type="button"
                      className={s.iconBtn}
                      onClick={() => renameCat.mutate()}
                      aria-label="저장"
                    >
                      <Check size={15} />
                    </button>
                    <button
                      type="button"
                      className={s.iconBtn}
                      onClick={() => setEditCat(null)}
                      aria-label="취소"
                    >
                      <X size={15} />
                    </button>
                  </div>
                ) : (
                  <div key={c.id} className={s.catRow}>
                    <span className={s.catName}>{c.name}</span>
                    <span className={s.catMeta}>
                      학생 {c.studentCount.toLocaleString()}명 · 규칙 {c.ruleCount}개
                    </span>
                    <div className={s.catActions}>
                      <button
                        type="button"
                        className={s.iconBtn}
                        onClick={() => setEditCat({ id: c.id, name: c.name })}
                        aria-label="이름 고치기"
                      >
                        <Pencil size={14} />
                      </button>
                      <button
                        type="button"
                        className={`${s.iconBtn} ${s.iconBtnDanger}`}
                        onClick={() => deleteCat.mutate(c.id)}
                        aria-label="지우기"
                      >
                        <Trash2 size={14} />
                      </button>
                    </div>
                  </div>
                ),
              )}
            </div>

            <div className={s.rowEnd}>
              <Field label="새 분류" hint="예: 1단지 · 가온마을5단지 · 주택">
                <Input
                  value={newCat}
                  onChange={(e) => setNewCat(e.target.value)}
                  onKeyDown={(e) => e.key === 'Enter' && newCat.trim() && createCat.mutate()}
                  placeholder="분류 이름"
                  maxLength={30}
                />
              </Field>
              <Button icon={Plus} onClick={() => createCat.mutate()} disabled={!newCat.trim()}>
                추가
              </Button>
            </div>

            <Notice tone="info">
              쓰고 있는 분류는 지울 수 없습니다. 먼저 학생과 규칙을 다른 분류로 옮겨 주세요.
            </Notice>
          </div>
        </Card>

        {/* ---------- 규칙 ---------- */}
        <Card
          title="주소 규칙"
          description="주소를 보고 어느 분류인지 정하는 규칙입니다. 위에 있는 종류가 먼저 적용됩니다."
        >
          <div className={s.stack}>
            <ErrorNotice
              error={createRule.error ?? saveRule.error ?? deleteRule.error ?? applyRule.error}
            />

            {madeRule && (
              <Notice tone={madeRule.matching.unclassified > 0 ? 'info' : 'success'}>
                규칙을 저장했습니다.{' '}
                {madeRule.matching.unclassified > 0 ? (
                  <>
                    이 규칙과 일치하는 <b>미분류 학생이 {madeRule.matching.unclassified}명</b>{' '}
                    있습니다.
                    {madeRule.matching.manual > 0 &&
                      ` (직접 지정한 ${madeRule.matching.manual}명은 건드리지 않습니다.)`}
                    <div style={{ marginTop: 8, display: 'flex', gap: 8 }}>
                      <Button
                        size="sm"
                        variant="primary"
                        onClick={() => applyRule.mutate(madeRule.ruleId)}
                        disabled={applyRule.isPending}
                      >
                        {madeRule.matching.unclassified}명에게 적용
                      </Button>
                      <Button size="sm" variant="outline" onClick={() => setMadeRule(null)}>
                        규칙만 저장
                      </Button>
                    </div>
                  </>
                ) : (
                  '지금 이 규칙에 걸리는 미분류 학생은 없습니다.'
                )}
              </Notice>
            )}

            <div className={s.ruleForm}>
              <Field label="종류">
                <Select value={kind} onChange={(e) => setKind(e.target.value as RuleKind)}>
                  {KINDS.map((k) => (
                    <option key={k} value={k}>
                      {RULE_KIND_LABEL[k]}
                    </option>
                  ))}
                </Select>
              </Field>
              <Field label="주소 규칙">
                <Input
                  value={pattern}
                  onChange={(e) => setPattern(e.target.value)}
                  placeholder={
                    kind === 'ROAD'
                      ? '예: ○○로 123'
                      : kind === 'COMPLEX'
                        ? '예: 가온마을5단지'
                        : '예: ○○빌라'
                  }
                  maxLength={100}
                />
              </Field>
              <Field label="분류">
                <Select
                  value={catId ?? ''}
                  onChange={(e) => setCatId(Number(e.target.value))}
                >
                  {cats.map((c) => (
                    <option key={c.id} value={c.id}>
                      {c.name}
                    </option>
                  ))}
                </Select>
              </Field>
              <Button
                variant="primary"
                icon={Plus}
                onClick={() => createRule.mutate()}
                disabled={!pattern.trim() || catId == null || createRule.isPending}
              >
                규칙 추가
              </Button>
              <div className={s.kindHint}>{RULE_KIND_HINT[kind]}</div>
            </div>

            {(rules.data ?? []).length === 0 ? (
              <Empty
                icon={MapPinned}
                title="아직 주소 규칙이 없습니다"
                description="학생명단에서 미분류 학생을 열어 분류를 정하면 그 자리에서 규칙으로 저장할 수 있습니다."
              />
            ) : (
              <TableWrap>
                <thead>
                  <tr>
                    <th style={{ width: 82 }}>종류</th>
                    <th>주소 규칙</th>
                    <th style={{ width: 130 }}>분류</th>
                    <th style={{ width: 74 }} className={tableClass.num}>
                      적용
                    </th>
                    <th style={{ width: 72 }}>사용</th>
                    <th style={{ width: 80 }} />
                  </tr>
                </thead>
                <tbody>
                  {(rules.data ?? []).map((r) =>
                    editRule?.id === r.id ? (
                      <tr key={r.id} style={{ cursor: 'default' }}>
                        <td>
                          <Select
                            value={editRule.kind}
                            onChange={(e) =>
                              setEditRule({ ...editRule, kind: e.target.value as RuleKind })
                            }
                          >
                            {KINDS.map((k) => (
                              <option key={k} value={k}>
                                {RULE_KIND_LABEL[k]}
                              </option>
                            ))}
                          </Select>
                        </td>
                        <td>
                          <Input
                            value={editRule.pattern}
                            onChange={(e) =>
                              setEditRule({ ...editRule, pattern: e.target.value })
                            }
                          />
                        </td>
                        <td>
                          <Select
                            value={editRule.categoryId}
                            onChange={(e) =>
                              setEditRule({ ...editRule, categoryId: Number(e.target.value) })
                            }
                          >
                            {cats.map((c) => (
                              <option key={c.id} value={c.id}>
                                {c.name}
                              </option>
                            ))}
                          </Select>
                        </td>
                        <td colSpan={2} />
                        <td>
                          <div className={s.catActions}>
                            <button
                              type="button"
                              className={s.iconBtn}
                              onClick={() => saveRule.mutate()}
                              aria-label="저장"
                            >
                              <Check size={15} />
                            </button>
                            <button
                              type="button"
                              className={s.iconBtn}
                              onClick={() => setEditRule(null)}
                              aria-label="취소"
                            >
                              <X size={15} />
                            </button>
                          </div>
                        </td>
                      </tr>
                    ) : (
                      <tr
                        key={r.id}
                        style={{ cursor: 'default' }}
                        className={r.isActive ? undefined : s.inactive}
                      >
                        <td>
                          <Badge tone={r.kind === 'CONTAINS' ? 'warn' : 'info'}>
                            {r.kindLabel}
                          </Badge>
                        </td>
                        <td className={s.patternCell}>{r.pattern}</td>
                        <td>{r.categoryName}</td>
                        <td className={s.appliedCell}>{r.applied.toLocaleString()}명</td>
                        <td>
                          <Button size="sm" variant="ghost" onClick={() => toggleRule.mutate(r)}>
                            {r.isActive ? '사용 중' : '사용 안 함'}
                          </Button>
                        </td>
                        <td>
                          <div className={s.catActions}>
                            <button
                              type="button"
                              className={s.iconBtn}
                              onClick={() => setEditRule(r)}
                              aria-label="고치기"
                            >
                              <Pencil size={14} />
                            </button>
                            <button
                              type="button"
                              className={`${s.iconBtn} ${s.iconBtnDanger}`}
                              onClick={() => deleteRule.mutate(r.id)}
                              aria-label="지우기"
                            >
                              <Trash2 size={14} />
                            </button>
                          </div>
                        </td>
                      </tr>
                    ),
                  )}
                </tbody>
              </TableWrap>
            )}

            <Notice tone="info">
              규칙을 고치거나 지운 뒤에는 오른쪽 위 [주소 규칙 다시 적용]을 눌러 주세요.
              직접 지정한 학생은 다시 적용해도 바뀌지 않습니다.
            </Notice>
          </div>
        </Card>
      </div>
    </Page>
  )
}
