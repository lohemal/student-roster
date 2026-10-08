import { useMutation, useQuery } from '@tanstack/react-query'
import { Trash2 } from 'lucide-react'

import { Button, ErrorNotice, Notice } from '@/components/ui'
import { settingsApi, type YearDeleteImpact } from '@/ipc/settings'
import s from './YearDelete.module.css'

const n = (v: number) => v.toLocaleString('ko-KR')

interface Props {
  year: number
  onClose: () => void
  onDone: (message: string) => void
}

/**
 * 학년도 삭제.
 *
 * 되돌리기 어려운 작업이라 **세 가지를 먼저 보여 준다** — 무엇이 지워지는지,
 * 무엇이 남는지, 지우기 전에 백업이 만들어진다는 것. 그러고 나서 사람이 누른다.
 *
 * 숫자는 실제로 센 것만 보여 준다. 0 인 항목은 그리지 않는다 — 없는 것을 0 으로
 * 늘어놓으면 "이것도 지워지나" 하고 읽게 된다.
 */
export function YearDeleteDialog({ year, onClose, onDone }: Props) {
  const impact = useQuery({
    queryKey: ['year-delete-impact', year],
    queryFn: () => settingsApi.yearDeleteImpact(year),
  })

  const run = useMutation({
    mutationFn: () => settingsApi.deleteYear(year),
    onSuccess: (r) =>
      onDone(
        `${r.year}학년도를 삭제했습니다. 학적 ${n(r.enrollments)}건이 사라졌고, ` +
          `삭제 전 백업을 만들어 두었습니다.`,
      ),
  })

  const d = impact.data
  const busy = run.isPending

  return (
    <div className={s.overlay} onClick={() => !busy && onClose()}>
      <div className={s.box} role="dialog" aria-modal="true" onClick={(e) => e.stopPropagation()}>
        <header className={s.head}>
          <h2 className={s.title}>{year}학년도 삭제</h2>
          <p className={s.sub}>
            {year}학년도와 이 학년도에만 속하는 학적 및 관련 기록이 삭제됩니다. 다른
            학년도의 학생정보는 그대로 유지됩니다.
          </p>
        </header>

        <div className={s.body}>
          <ErrorNotice error={impact.error ?? run.error} />

          {!d ? (
            <div className={s.hint}>세어 보는 중…</div>
          ) : !d.deletable ? (
            <Notice tone="error">{d.refusalMessage}</Notice>
          ) : (
            <>
              <Group title="삭제되는 데이터">
                <Line label="학생 학적" value={d.enrollments} strong />
                <Line label="학적 이동 · 진급 기록" value={d.events} hint={moveHint(d)} />
                <Line label="번호 변경 기록" value={d.renumberOps} />
                <Line label="엑셀 가져오기 기록" value={d.imports} />
                <Line label="학년도 전환 기록" value={d.transitions} />
                <Line label="학년도" value={1} unit="개" />
              </Group>

              <Group title="보존되는 데이터">
                <Line label="학생 기본정보 · 주소 · 보호자" value={d.students} unit="명" muted />
                <Line label="다른 학년도 학적" value={d.otherEnrollments} muted />
                <Line label="졸업 기록" value={d.graduations} muted />
                <Line label="형제 관계" value={d.siblingLinks} muted />
              </Group>

              {d.issues > 0 && (
                <Notice tone="info">
                  이 학년도 학생의 <b>확인 필요 {n(d.issues)}건</b>은 남은 학년도 자료로
                  다시 계산합니다. 확인 필요는 자료에서 나온 결과이지 자료가 아닙니다.
                </Notice>
              )}

              {d.graduationsFromTransition > 0 && d.graduationYear != null && (
                <Notice tone="warn">
                  이 학년도를 만든 전환이 <b>{d.graduationYear}학년도 졸업 기록{' '}
                  {n(d.graduationsFromTransition)}건</b>도 함께 만들었습니다. 졸업은 그
                  학년도에 실제로 일어난 일이므로 <b>지우지 않습니다.</b> 되돌리려면
                  졸업생 화면에서 졸업 취소를 해 주세요.
                </Notice>
              )}

              {d.studentsWithoutOtherYear > 0 && (
                <Notice tone="info">
                  <b>{n(d.studentsWithoutOtherYear)}명</b>은 이 학년도에만 학적이 있어
                  삭제 후 어느 명단에도 나오지 않습니다. 그래도 <b>학생 기본정보는 지우지
                  않습니다</b> — 이 기능은 학년도 삭제이지 학생 삭제가 아닙니다.
                </Notice>
              )}

              <Notice tone="info">
                삭제 직전에 <b>자동 백업</b>이 만들어집니다. 이 백업은 자동 정리 대상이
                아니어서 그대로 남습니다.
              </Notice>
            </>
          )}
        </div>

        <footer className={s.foot}>
          <span className={s.spacer} />
          <Button variant="outline" onClick={onClose} disabled={busy}>
            취소
          </Button>
          <Button
            variant="danger"
            icon={Trash2}
            disabled={busy || !d?.deletable}
            onClick={() => run.mutate()}
          >
            {busy ? '삭제하는 중…' : `${year}학년도 삭제`}
          </Button>
        </footer>
      </div>
    </div>
  )
}

/** 전입·전출이 섞여 있으면 몇 건인지 따로 알려 준다 */
function moveHint(d: YearDeleteImpact): string | undefined {
  return d.moves > 0 ? `전입 · 전출 ${n(d.moves)}건 포함` : undefined
}

function Group({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className={s.group}>
      <div className={s.groupTitle}>{title}</div>
      <div className={s.lines}>{children}</div>
    </div>
  )
}

function Line({
  label,
  value,
  unit = '건',
  hint,
  strong,
  muted,
}: {
  label: string
  value: number
  unit?: string
  hint?: string
  strong?: boolean
  muted?: boolean
}) {
  // 0 인 항목은 그리지 않는다 — 실제로 있는 것만 보여 준다
  if (value === 0) return null
  const cls = [s.line, strong ? s.lineStrong : '', muted ? s.lineMuted : ''].filter(Boolean)
  return (
    <div className={cls.join(' ')}>
      <span>
        {label}
        {hint && <span className={s.lineHint}> {hint}</span>}
      </span>
      <span className={s.lineNum}>
        {n(value)}
        {unit}
      </span>
    </div>
  )
}
