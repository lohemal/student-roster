import { useState } from 'react'
import { useMutation, useQuery } from '@tanstack/react-query'
import { save } from '@tauri-apps/plugin-dialog'
import { FileSpreadsheet } from 'lucide-react'

import { Button, ErrorNotice, Notice } from '@/components/ui'
import { statsApi, type StatFilter, type StatsSheetKey } from '@/ipc/stats'
import s from './StatsExport.module.css'

const n = (v: number) => v.toLocaleString('ko-KR')

interface Props {
  /** 화면에 걸려 있는 조건 그대로 — 파일도 같은 조건으로 센다 */
  filter: StatFilter
  /** 조건을 한 줄로 (`3학년 · 5단지`). 조건이 없으면 빈 글자. */
  who: string
  onClose: () => void
  onDone: (message: string) => void
}

/**
 * 통계 Excel 다운로드.
 *
 * **화면에서 계산한 숫자를 그대로 내보낸다.** 집계를 Excel 용으로 다시 만들지 않고
 * 화면과 같은 명령·같은 조건을 지난다. 고른 통계마다 시트 한 장, 파일은 하나다 —
 * 통계는 나란히 놓고 견주는 자료이기 때문이다.
 */
export function StatsExportDialog({ filter, who, onClose, onDone }: Props) {
  const sheets = useQuery({ queryKey: ['stats-export-sheets'], queryFn: statsApi.exportSheets })
  const [picked, setPicked] = useState<StatsSheetKey[]>(['GRADE', 'CLASS', 'ADDRESS'])
  const [runError, setRunError] = useState<unknown>(null)

  const preview = useQuery({
    queryKey: ['stats-export-preview', filter, picked],
    queryFn: () => statsApi.exportPreview(filter, picked),
    enabled: picked.length > 0,
  })

  const toggle = (key: StatsSheetKey) =>
    setPicked((p) => (p.includes(key) ? p.filter((k) => k !== key) : [...p, key]))

  const all = sheets.data?.map((x) => x.key) ?? []
  const allOn = all.length > 0 && all.every((k) => picked.includes(k))

  const run = useMutation({
    mutationFn: async () => {
      const p = preview.data
      if (!p) return null
      // 저장 위치는 **사용자가 고른다.** 찾기 어려운 곳에 말없이 두지 않는다.
      const target = await save({
        defaultPath: `${p.defaultFileName}.xlsx`,
        filters: [{ name: 'Excel 통합 문서', extensions: ['xlsx'] }],
      })
      // 취소는 오류가 아니다
      if (!target) return null
      return statsApi.exportRun(filter, picked, target)
    },
    onSuccess: (r) => {
      if (r) onDone(`통계 ${r.sheets}장을 ${r.path} 에 저장했습니다.`)
    },
    onError: (e) => setRunError(e),
  })

  const busy = run.isPending

  return (
    <div className={s.overlay} onClick={() => !busy && onClose()}>
      <div className={s.box} role="dialog" aria-modal="true" onClick={(e) => e.stopPropagation()}>
        <header className={s.head}>
          <h2 className={s.title}>통계 Excel 다운로드</h2>
          <p className={s.sub}>
            지금 화면에 보이는 숫자를 그대로 내보냅니다. 고른 통계마다 시트 한 장씩,
            파일은 하나입니다.
          </p>
        </header>

        <div className={s.body}>
          <ErrorNotice error={sheets.error ?? preview.error ?? runError} />

          <div className={s.group}>
            <div className={s.groupHead}>
              <span className={s.groupTitle}>내보낼 통계</span>
              <Button
                size="sm"
                variant="ghost"
                onClick={() => setPicked(allOn ? [] : all)}
                disabled={busy || all.length === 0}
              >
                {allOn ? '전체 해제' : '전체 선택'}
              </Button>
            </div>
            <div className={s.choices}>
              {(sheets.data ?? []).map((x) => (
                <label key={x.key} className={s.choice}>
                  <input
                    type="checkbox"
                    checked={picked.includes(x.key)}
                    onChange={() => toggle(x.key)}
                    disabled={busy}
                  />
                  <span>{x.label}</span>
                </label>
              ))}
            </div>
          </div>

          {picked.length === 0 ? (
            <Notice tone="warn">하나 이상 골라 주세요.</Notice>
          ) : preview.data ? (
            <>
              <div className={s.plan}>
                <PlanLine label="파일 이름" value={`${preview.data.defaultFileName}.xlsx`} />
                <PlanLine label="시트" value={preview.data.sheetNames.join(' · ')} />
                <PlanLine label="조건" value={who || '전체'} />
                <PlanLine label="센 학생" value={`${n(preview.data.students)}명`} strong />
              </div>
              <p className={s.hint}>
                학생 이름과 연락처는 들어가지 않습니다. 인원만 담긴 표입니다.
              </p>
            </>
          ) : (
            <div className={s.hint}>세어 보는 중…</div>
          )}
        </div>

        <footer className={s.foot}>
          <span className={s.spacer} />
          <Button variant="outline" onClick={onClose} disabled={busy}>
            취소
          </Button>
          <Button
            variant="primary"
            icon={FileSpreadsheet}
            disabled={busy || picked.length === 0 || !preview.data}
            onClick={() => {
              setRunError(null)
              run.mutate()
            }}
          >
            {busy ? '만드는 중…' : 'Excel 다운로드'}
          </Button>
        </footer>
      </div>
    </div>
  )
}

function PlanLine({
  label,
  value,
  strong,
}: {
  label: string
  value: string
  strong?: boolean
}) {
  return (
    <div className={strong ? `${s.planLine} ${s.planStrong}` : s.planLine}>
      <span>{label}</span>
      <span className={s.planVal}>{value}</span>
    </div>
  )
}
