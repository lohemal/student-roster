import { useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { AlertTriangle, MapPinned } from 'lucide-react'

import {
  Badge,
  Button,
  ErrorNotice,
  Field,
  FieldAction,
  Select,
  type Tone,
} from '@/components/ui'
import {
  addressApi,
  RULE_KIND_LABEL,
  type AddressSource,
  type Matching,
  type Suggestion,
} from '@/ipc/address'
import s from './AddressPanel.module.css'

const SOURCE_TONE: Record<AddressSource, Tone> = {
  MANUAL: 'info',
  RULE: 'success',
  AUTO: 'success',
  CONFLICT: 'error',
  NONE: 'warn',
}

interface Props {
  studentId: number
  schoolYear: number
  /** 지금 입력칸에 있는 주소. 저장하지 않은 변경이 있으면 알려 준다 */
  currentAddress: string
  /** 저장된 주소 */
  savedAddress: string
  onChanged: () => void
}

/**
 * 학생 상세의 주소 분류 칸.
 *
 * 분류만 보여 주지 않고 **왜 그렇게 정해졌는지**와, 미분류라면 바로 정할 수 있는 길을 준다.
 */
export function AddressPanel({
  studentId,
  schoolYear,
  currentAddress,
  savedAddress,
  onChanged,
}: Props) {
  const qc = useQueryClient()
  const status = useQuery({
    queryKey: ['address-status', studentId],
    queryFn: () => addressApi.status(studentId),
  })

  const [pick, setPick] = useState<number | ''>('')
  const [madeRule, setMadeRule] = useState<{ ruleId: number; matching: Matching } | null>(null)

  const categories = useQuery({
    queryKey: ['address-categories', schoolYear],
    queryFn: () => addressApi.categories(schoolYear),
  })

  const refresh = () => {
    qc.invalidateQueries({ queryKey: ['address-status', studentId] })
    qc.invalidateQueries({ queryKey: ['address-rules'] })
    qc.invalidateQueries({ queryKey: ['address-categories'] })
    onChanged()
  }

  const setManual = useMutation({
    mutationFn: (categoryId: number | null) =>
      addressApi.setManual(studentId, categoryId, schoolYear),
    onSuccess: () => {
      setPick('')
      refresh()
    },
  })

  const saveRule = useMutation({
    mutationFn: (sug: Suggestion) =>
      addressApi.createRule(
        { kind: sug.kind, pattern: sug.pattern, categoryId: Number(pick) },
        schoolYear,
      ),
    onSuccess: (r) => {
      setMadeRule(r)
      refresh()
    },
  })

  const applyRule = useMutation({
    mutationFn: (ruleId: number) => addressApi.applyRule(ruleId, schoolYear),
    onSuccess: () => {
      setMadeRule(null)
      refresh()
    },
  })

  const st = status.data
  if (!st) return null

  const dirty = currentAddress.trim() !== savedAddress.trim()
  const needsAttention = st.source === 'NONE' || st.source === 'CONFLICT'
  const hasAddress = savedAddress.trim() !== ''

  return (
    <div className={needsAttention && hasAddress ? `${s.panel} ${s.panelWarn}` : s.panel}>
      <div className={s.head}>
        <MapPinned size={15} color="var(--text-muted)" />
        <span className={s.title}>주소 분류</span>
        <Badge tone={SOURCE_TONE[st.source]}>{st.sourceLabel}</Badge>
      </div>

      <div className={s.category}>
        {st.categoryName ?? <span className={s.unclassified}>미분류</span>}
      </div>
      <div className={s.reason}>{st.reason}</div>
      {st.ruleText && <div className={s.ruleText}>{st.ruleText}</div>}

      {st.conflicts.length > 0 && (
        <div className={s.conflicts}>
          {st.conflicts.map((h) => (
            <span key={h.ruleId}>
              <AlertTriangle size={11} style={{ verticalAlign: -1, marginRight: 4 }} />
              {RULE_KIND_LABEL[h.kind as keyof typeof RULE_KIND_LABEL] ?? h.kind} · {h.pattern} →{' '}
              {h.categoryName}
            </span>
          ))}
        </div>
      )}

      {dirty && (
        <div className={s.reason}>
          주소를 고쳤습니다. <b>저장</b>하면 새 주소로 다시 판정합니다.
        </div>
      )}

      <ErrorNotice error={setManual.error ?? saveRule.error ?? applyRule.error} />

      {/* 분류 고르기 */}
      {hasAddress && (
        <div className={s.actions}>
          <Field label="분류 지정" className={s.pick}>
            <Select value={pick} onChange={(e) => setPick(Number(e.target.value) || '')}>
              <option value="">분류를 고르세요</option>
              {(categories.data ?? []).map((c) => (
                <option key={c.id} value={c.id}>
                  {c.name}
                </option>
              ))}
            </Select>
          </Field>
          <FieldAction>
            <Button
              size="sm"
              variant="primary"
              onClick={() => setManual.mutate(Number(pick))}
              disabled={pick === '' || setManual.isPending}
            >
              이번 학생만 적용
            </Button>
          </FieldAction>
          {st.source === 'MANUAL' && (
            <FieldAction>
              <Button
                size="sm"
                variant="ghost"
                onClick={() => setManual.mutate(null)}
                disabled={setManual.isPending}
              >
                직접 지정 풀기
              </Button>
            </FieldAction>
          )}
        </div>
      )}

      {/* 규칙으로 저장 */}
      {pick !== '' && st.suggestions.length > 0 && !madeRule && (
        <div className={s.suggestion}>
          <div className={s.suggestionHead}>이 주소를 규칙으로 저장하면 다른 학생에게도 적용됩니다</div>
          {st.suggestions.map((sug) => (
            <div key={`${sug.kind}-${sug.pattern}`}>
              <div className={s.suggestionRow}>
                <Badge tone="info">{sug.kindLabel}</Badge>
                <span className={s.suggestionPattern}>{sug.pattern}</span>
                <span>→</span>
                <span>
                  {(categories.data ?? []).find((c) => c.id === Number(pick))?.name ?? ''}
                </span>
                <Button
                  size="sm"
                  variant="outline"
                  onClick={() => saveRule.mutate(sug)}
                  disabled={saveRule.isPending}
                >
                  규칙으로 저장
                </Button>
              </div>
              <div className={s.suggestionHint}>{sug.hint}</div>
            </div>
          ))}
        </div>
      )}

      {/* 규칙을 만든 뒤 — 같은 주소 학생에게 함께 적용 */}
      {madeRule && (
        <div className={s.suggestion}>
          <div className={s.suggestionHead}>규칙을 저장했습니다</div>
          {madeRule.matching.unclassified > 0 ? (
            <>
              <div className={s.matching}>
                이 규칙과 일치하는 <b>미분류 학생이 {madeRule.matching.unclassified}명</b> 있습니다.
                {madeRule.matching.manual > 0 &&
                  ` 직접 지정한 ${madeRule.matching.manual}명은 건드리지 않습니다.`}
              </div>
              <div className={s.suggestionRow}>
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
            <div className={s.matching}>지금 이 규칙에 걸리는 미분류 학생은 없습니다.</div>
          )}
        </div>
      )}
    </div>
  )
}
