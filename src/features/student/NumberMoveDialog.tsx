import { useEffect } from 'react'

import { Button, ErrorNotice, Notice } from '@/components/ui'
import type { RenumberPreview } from '@/ipc/renumber'
import s from './NumberMoveDialog.module.css'

interface Props {
  preview: RenumberPreview
  /** 저장 중 */
  busy?: boolean
  error?: unknown
  onCancel: () => void
  onApply: () => void
  /** 번호가 겹쳐 막혔을 때 확인 필요 화면으로 보내 준다 */
  onSeeDuplicates?: () => void
}

/**
 * 번호를 바꾸기 전에 무슨 일이 생기는지 보여 준다.
 *
 * **번호 하나를 바꿨더니 모르는 사이에 다른 열여섯 명의 번호가 바뀌는 일은 없어야 한다.**
 * 그래서 바뀌는 학생을 모두 적어 두고, 사용자가 [번호 이동]을 눌러야 저장한다.
 */
export function NumberMoveDialog({
  preview,
  busy,
  error,
  onCancel,
  onApply,
  onSeeDuplicates,
}: Props) {
  // Esc 로 닫기 — 저장 중에는 닫지 않는다
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && !busy) onCancel()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [busy, onCancel])

  const blocked = preview.blocked != null

  return (
    <div className={s.overlay} onClick={() => !busy && onCancel()}>
      <div
        className={s.box}
        role="dialog"
        aria-modal="true"
        aria-label="학생 번호 변경"
        onClick={(e) => e.stopPropagation()}
      >
        <header className={s.head}>
          <h2 className={s.title}>학생 번호 변경</h2>
          <div className={s.who}>
            {preview.name} · {preview.classLabel}
          </div>
        </header>

        <div className={s.body}>
          <div className={s.jump}>
            <span className={s.jumpFrom}>
              {preview.from != null ? `${preview.from}번` : '번호 없음'}
            </span>
            <span className={s.jumpArrow}>→</span>
            <span className={s.jumpTo}>{preview.to}번</span>
          </div>

          <ErrorNotice error={error} />

          {blocked ? (
            <Notice tone="warn">{preview.blocked}</Notice>
          ) : (
            <>
              <div className={s.summary}>{preview.summary}</div>

              {preview.rows.length > 0 && (
                <div className={s.tableWrap}>
                  <table className={s.table}>
                    <thead>
                      <tr>
                        <th>학생</th>
                        <th className={s.num}>기존</th>
                        <th className={s.arrowCell} aria-hidden="true" />
                        <th className={s.num}>변경</th>
                      </tr>
                    </thead>
                    <tbody>
                      {preview.rows.map((r) => (
                        <tr key={r.studentId} className={r.isTarget ? s.target : undefined}>
                          <td>{r.name}</td>
                          <td className={s.num}>{r.from ?? '—'}</td>
                          <td className={s.arrowCell}>→</td>
                          <td className={s.num}>{r.to}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              )}

              <div className={s.hint}>
                같은 학년·반 학생만 바뀝니다. 다른 반 학생의 번호는 그대로입니다.
              </div>
            </>
          )}
        </div>

        <footer className={s.foot}>
          {blocked && onSeeDuplicates && (
            <Button size="sm" variant="ghost" onClick={onSeeDuplicates}>
              번호 중복 보기
            </Button>
          )}
          <span className={s.footSpacer} />
          <Button variant="outline" onClick={onCancel} disabled={busy}>
            취소
          </Button>
          {!blocked && (
            <Button variant="primary" onClick={onApply} disabled={busy}>
              {busy ? '바꾸는 중…' : '번호 이동'}
            </Button>
          )}
        </footer>
      </div>
    </div>
  )
}
