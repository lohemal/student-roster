import { useEffect, useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { check } from '@tauri-apps/plugin-updater'
import { useNavigate } from 'react-router-dom'
import { AlertTriangle, ArrowRight, CheckCircle2, Download, X } from 'lucide-react'

import { appApi } from '@/ipc/app'
import s from './system.module.css'

/**
 * 프로그램을 켤 때 알려야 할 것 — 자료 복원 결과, 무결성 문제, 새 버전.
 *
 * 자료에 문제가 있다는 사실은 설정 화면까지 들어가야 보이면 안 된다. 어느 화면에 있든
 * 눈에 들어오게 위에 한 줄로 띄운다. 업데이트는 **막지 않는다** — 확인에 실패해도
 * (오프라인이어도) 아무 일도 일어나지 않는다.
 */
export function SystemBanner() {
  const nav = useNavigate()
  const info = useQuery({ queryKey: ['app-info'], queryFn: appApi.info })
  const [hidden, setHidden] = useState<string[]>([])
  const [newVersion, setNewVersion] = useState<string | null>(null)

  // 하루 한 번만 살펴본다. 실패는 조용히 넘긴다 — 프로그램 사용을 막지 않는다.
  useEffect(() => {
    let alive = true
    ;(async () => {
      try {
        if (!(await appApi.updateCheckDue())) return
        const found = await check()
        await appApi.updateMarkChecked().catch(() => {})
        if (alive && found) setNewVersion(found.version)
      } catch {
        // 오프라인이거나 아직 배포된 버전이 없다 — 알릴 일이 아니다
      }
    })()
    return () => {
      alive = false
    }
  }, [])

  const notes = (info.data?.notes ?? []).filter((x) => !hidden.includes(x.kind))
  if (notes.length === 0 && !newVersion) return null

  return (
    <div className={s.banners}>
      {notes.map((note) => (
        <div
          key={note.kind}
          className={`${s.banner} ${note.tone === 'error' ? s.bannerBad : note.tone === 'success' ? s.bannerGood : s.bannerWarn}`}
        >
          {note.tone === 'success' ? (
            <CheckCircle2 size={16} className={s.bannerIcon} />
          ) : (
            <AlertTriangle size={16} className={s.bannerIcon} />
          )}
          <span className={s.bannerText}>{note.message}</span>
          {note.kind !== 'RESTORED' && (
            <button className={s.bannerLink} onClick={() => nav('/settings')}>
              설정에서 백업 보기 <ArrowRight size={13} />
            </button>
          )}
          <button
            className={s.bannerClose}
            aria-label="닫기"
            onClick={() => setHidden((v) => [...v, note.kind])}
          >
            <X size={14} />
          </button>
        </div>
      ))}

      {newVersion && (
        <div className={`${s.banner} ${s.bannerInfo}`}>
          <Download size={16} className={s.bannerIcon} />
          <span className={s.bannerText}>
            새 버전 {newVersion} 을 사용할 수 있습니다. 지금 하던 일을 마친 뒤에 설치하세요.
          </span>
          <button className={s.bannerLink} onClick={() => nav('/settings')}>
            업데이트 <ArrowRight size={13} />
          </button>
          <button
            className={s.bannerClose}
            aria-label="닫기"
            onClick={() => setNewVersion(null)}
          >
            <X size={14} />
          </button>
        </div>
      )}
    </div>
  )
}
