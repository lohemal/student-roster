import { useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { AlertTriangle, Check, Users, X } from 'lucide-react'

import { Badge, Button, Empty, ErrorNotice, Notice, type Tone } from '@/components/ui'
import {
  siblingApi,
  type GuardianField,
  type SiblingStatus,
  type SiblingView,
} from '@/ipc/sibling'
import s from './SiblingPanel.module.css'

const STATUS_TONE: Record<SiblingStatus, Tone> = {
  CANDIDATE: 'warn',
  CONFIRMED: 'success',
  REJECTED: 'neutral',
}

interface Props {
  studentId: number
  studentName: string
  schoolYear: number
  onChanged: () => void
}

/**
 * 학생 상세의 형제 칸.
 *
 * 프로그램은 후보와 그 근거만 보여 주고, 형제인지 아닌지는 사람이 정한다.
 * 확정한 뒤에는 보호자 정보의 빈칸을 채울지, 서로 다른 값을 어떻게 할지 묻는다.
 */
export function SiblingPanel({ studentId, studentName, schoolYear, onChanged }: Props) {
  const qc = useQueryClient()
  const list = useQuery({
    queryKey: ['siblings', studentId, schoolYear],
    queryFn: () => siblingApi.list(studentId, schoolYear),
  })

  const refresh = () => {
    qc.invalidateQueries({ queryKey: ['siblings'] })
    qc.invalidateQueries({ queryKey: ['student', studentId] })
    onChanged()
  }

  const decide = useMutation({
    mutationFn: (args: { linkId: number; what: 'confirm' | 'reject' | 'reset' }) =>
      siblingApi[args.what](args.linkId, schoolYear),
    onSuccess: refresh,
  })

  const rows = list.data ?? []
  const confirmed = rows.filter((r) => r.status === 'CONFIRMED')
  const candidates = rows.filter((r) => r.status === 'CANDIDATE')
  const rejected = rows.filter((r) => r.status === 'REJECTED')

  if (list.isLoading) return <div className={s.empty}>불러오는 중…</div>

  return (
    <div className={s.wrap}>
      <ErrorNotice error={list.error ?? decide.error} />

      {rows.length === 0 && (
        <Empty
          icon={Users}
          title="본교에 형제로 보이는 학생이 없습니다"
          description="보호자 성명과 연락처 가운데 값이 있는 항목이 두 가지 이상 같은 학생을 찾습니다. 자료를 채운 뒤 [형제 후보 다시 찾기]를 눌러 보세요."
        />
      )}

      {confirmed.length > 0 && (
        <div className={s.group}>
          <div className={s.groupTitle}>본교 형제 {confirmed.length}명</div>
          {confirmed.map((v) => (
            <ConfirmedCard
              key={v.linkId}
              view={v}
              studentId={studentId}
              studentName={studentName}
              schoolYear={schoolYear}
              onChanged={refresh}
              onReset={() => decide.mutate({ linkId: v.linkId, what: 'reset' })}
            />
          ))}
        </div>
      )}

      {candidates.length > 0 && (
        <div className={s.group}>
          <div className={s.groupTitle}>형제 후보 {candidates.length}명</div>
          {candidates.map((v) => (
            <div key={v.linkId} className={`${s.card} ${s.cardCandidate}`}>
              <div className={s.head}>
                <span className={s.name}>{v.label}</span>
                {v.partnerNote && <Badge tone="neutral">{v.partnerNote}</Badge>}
                <Badge tone={STATUS_TONE[v.status]}>{v.statusLabel}</Badge>
              </div>
              <Reasons view={v} />
              <div className={s.actions}>
                <Button
                  size="sm"
                  variant="primary"
                  icon={Check}
                  onClick={() => decide.mutate({ linkId: v.linkId, what: 'confirm' })}
                  disabled={decide.isPending}
                >
                  형제로 확인
                </Button>
                <Button
                  size="sm"
                  variant="outline"
                  icon={X}
                  onClick={() => decide.mutate({ linkId: v.linkId, what: 'reject' })}
                  disabled={decide.isPending}
                >
                  형제 아님
                </Button>
              </div>
            </div>
          ))}
        </div>
      )}

      {rejected.length > 0 && (
        <div className={s.group}>
          <div className={s.groupTitle}>형제 아님 {rejected.length}명</div>
          {rejected.map((v) => (
            <div key={v.linkId} className={`${s.card} ${s.cardRejected}`}>
              <div className={s.head}>
                <span className={s.name}>{v.label}</span>
                <Badge tone="neutral">{v.statusLabel}</Badge>
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={() => decide.mutate({ linkId: v.linkId, what: 'reset' })}
                  disabled={decide.isPending}
                >
                  다시 검토
                </Button>
              </div>
            </div>
          ))}
          <div className={s.empty}>
            형제가 아니라고 정한 관계는 다시 찾아도 후보로 올라오지 않습니다.
          </div>
        </div>
      )}
    </div>
  )
}

/** 무엇이 같고 무엇이 다른지 */
function Reasons({ view }: { view: SiblingView }) {
  return (
    <>
      <div className={s.matchCount}>
        값이 있는 항목 {view.matched.length}가지가 같습니다.
      </div>
      <div className={s.reasons}>
        {view.matched.map((f) => (
          <div key={f.key} className={s.reason}>
            <span className={s.reasonLabel}>{f.label}</span>
            <span className={s.ok}>✓ 같음</span>
            <span className={s.values}>{f.mine}</span>
          </div>
        ))}
        {view.conflicts.map((f) => (
          <div key={f.key} className={s.reason}>
            <span className={s.reasonLabel}>{f.label}</span>
            <span className={s.bad}>✕ 다름</span>
          </div>
        ))}
      </div>
    </>
  )
}

/** 확정된 형제 한 명 — 보호자 정보 보완·불일치까지 */
function ConfirmedCard({
  view,
  studentId,
  studentName,
  schoolYear,
  onChanged,
  onReset,
}: {
  view: SiblingView
  studentId: number
  studentName: string
  schoolYear: number
  onChanged: () => void
  onReset: () => void
}) {
  const [picked, setPicked] = useState<GuardianField[]>(() => view.fillable.map((f) => f.key))

  const fill = useMutation({
    mutationFn: () =>
      siblingApi.fill({ linkId: view.linkId, studentId, fields: picked, schoolYear }),
    onSuccess: onChanged,
  })

  const toggle = (key: GuardianField) =>
    setPicked((p) => (p.includes(key) ? p.filter((x) => x !== key) : [...p, key]))

  return (
    <div className={`${s.card} ${s.cardConfirmed}`}>
      <div className={s.head}>
        <span className={s.name}>{view.label}</span>
        {view.partnerNote && <Badge tone="neutral">{view.partnerNote}</Badge>}
        <Badge tone="success">{view.statusLabel}</Badge>
        <Button size="sm" variant="ghost" onClick={onReset}>
          관계 다시 검토
        </Button>
      </div>
      <Reasons view={view} />

      <ErrorNotice error={fill.error} />

      {view.fillable.length > 0 && (
        <div className={s.fill}>
          <div className={s.fillHead}>
            형제 학생에게 등록된 보호자 정보가 있습니다. 가져오시겠습니까?
          </div>
          {view.fillable.map((f) => (
            <label key={f.key} className={s.fillItem}>
              <input
                type="checkbox"
                checked={picked.includes(f.key)}
                onChange={() => toggle(f.key)}
              />
              <span className={s.fillLabel}>{f.label}</span>
              <span className={s.fillValue}>{f.theirs}</span>
            </label>
          ))}
          <div className={s.actions}>
            <Button
              size="sm"
              variant="primary"
              onClick={() => fill.mutate()}
              disabled={picked.length === 0 || fill.isPending}
            >
              {picked.length}개 가져오기
            </Button>
          </div>
          <div className={s.empty}>
            비어 있는 항목만 채웁니다. 이미 값이 있는 항목은 바뀌지 않습니다.
          </div>
        </div>
      )}

      {view.conflicts.length > 0 && (
        <div className={s.conflictBox}>
          <div className={s.conflictHead}>
            <AlertTriangle size={13} style={{ verticalAlign: -2, marginRight: 4 }} />
            형제 학생과 값이 서로 다릅니다
          </div>
          {view.conflicts.map((f) => (
            <div key={f.key} className={s.compareRow}>
              <span className={s.reasonLabel}>{f.label}</span>
              <div>
                <div className={s.compareSide}>
                  <span className={s.sideTag}>{studentName}</span>
                  <span className={s.values}>{f.mine}</span>
                </div>
                <div className={s.compareSide}>
                  <span className={s.sideTag}>{view.label.split(' ').pop()}</span>
                  <span className={s.values}>{f.theirs}</span>
                </div>
              </div>
            </div>
          ))}
          <div className={s.empty}>
            어느 쪽이 맞는지는 프로그램이 정하지 않습니다. 기본정보 탭에서 고쳐 주세요.
            값이 서로 다르다고 형제가 아닌 것은 아니므로 관계는 그대로 둡니다.
          </div>
        </div>
      )}

      {view.fillable.length === 0 && view.conflicts.length === 0 && (
        <Notice tone="success">보호자 정보가 형제와 어긋나는 곳이 없습니다.</Notice>
      )}
    </div>
  )
}
