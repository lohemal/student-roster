import { useState } from 'react'
import { useMutation, useQuery } from '@tanstack/react-query'
import { check, type Update } from '@tauri-apps/plugin-updater'
import { RefreshCw } from 'lucide-react'

import { Button, Card, Notice } from '@/components/ui'
import { appApi } from '@/ipc/app'
import s from './system.module.css'

type Phase =
  | { kind: 'IDLE' }
  | { kind: 'CHECKING' }
  | { kind: 'LATEST' }
  | { kind: 'FOUND'; update: Update }
  | { kind: 'DOWNLOADING'; got: number; total: number | null }
  | { kind: 'INSTALLING' }
  | { kind: 'FAILED'; message: string }

/** 사용자가 읽을 수 있는 말로. 원문은 접기 안에 둔다. */
function explain(e: unknown): string {
  const raw = e instanceof Error ? e.message : String(e)
  const low = raw.toLowerCase()
  if (low.includes('network') || low.includes('dns') || low.includes('connect')) {
    return '인터넷에 연결하지 못했습니다. 연결 상태를 확인한 뒤 다시 시도해 주세요.'
  }
  if (low.includes('signature') || low.includes('verify')) {
    return '내려받은 파일의 서명이 맞지 않아 설치하지 않았습니다. 잠시 뒤 다시 시도해 주세요.'
  }
  if (low.includes('404') || low.includes('not found')) {
    return '업데이트 정보를 찾지 못했습니다. 아직 배포된 새 버전이 없을 수 있습니다.'
  }
  return '업데이트를 하지 못했습니다. 잠시 뒤 다시 시도해 주세요.'
}

const kb = (v: number) => Math.round(v / 1024).toLocaleString('ko-KR')

/**
 * 프로그램 업데이트.
 *
 * ## `relaunch()` 를 부르지 않는다
 *
 * Windows 에서 `install()` 은 **돌아오지 않는다.** 설치 프로그램을 띄우고 앱을 스스로
 * 끝내며, 설치가 끝나면 NSIS 가 앱을 다시 켠다. 여기서 `relaunch()` 를 부르면 방금 뜬
 * 설치 프로그램과 경쟁해 파일을 바꾸지 못하고 구 버전이 그대로 남는다.
 *
 * ## 사람이 눌러야 시작한다
 *
 * 확인만으로 설치하지 않는다. 업무 중에 프로그램이 갑자기 닫히면 안 된다.
 * 업데이트 확인에 실패해도 학생명단 기능은 모두 그대로 쓸 수 있다.
 */
export function UpdateCard() {
  const info = useQuery({ queryKey: ['app-info'], queryFn: appApi.info })
  const [phase, setPhase] = useState<Phase>({ kind: 'IDLE' })
  const [detail, setDetail] = useState<string | null>(null)

  const fail = (e: unknown) => {
    setDetail(e instanceof Error ? e.message : String(e))
    setPhase({ kind: 'FAILED', message: explain(e) })
  }

  const look = useMutation({
    mutationFn: async () => {
      setDetail(null)
      setPhase({ kind: 'CHECKING' })
      const found = await check()
      await appApi.updateMarkChecked().catch(() => {})
      return found
    },
    onSuccess: (update) => setPhase(update ? { kind: 'FOUND', update } : { kind: 'LATEST' }),
    onError: fail,
  })

  const run = useMutation({
    mutationFn: async (update: Update) => {
      let got = 0
      let total: number | null = null
      setPhase({ kind: 'DOWNLOADING', got: 0, total: null })

      await update.download((ev) => {
        if (ev.event === 'Started') {
          total = ev.data.contentLength ?? null
          setPhase({ kind: 'DOWNLOADING', got: 0, total })
        } else if (ev.event === 'Progress') {
          got += ev.data.chunkLength
          setPhase({ kind: 'DOWNLOADING', got, total })
        }
      })

      // 설치 — 여기서 앱이 스스로 닫히고, 설치가 끝나면 다시 켜진다.
      // 아래 줄 다음은 실행되지 않는다 (Windows).
      setPhase({ kind: 'INSTALLING' })
      await update.install()
    },
    onError: fail,
  })

  const busy =
    phase.kind === 'CHECKING' || phase.kind === 'DOWNLOADING' || phase.kind === 'INSTALLING'

  return (
    <Card
      title="프로그램 업데이트"
      description={info.data ? `지금 v${info.data.appVersion}` : '새 버전이 있는지 확인합니다'}
      actions={
        <Button
          variant="primary"
          icon={RefreshCw}
          disabled={busy}
          onClick={() => look.mutate()}
        >
          {phase.kind === 'CHECKING' ? '확인하는 중…' : '업데이트 확인'}
        </Button>
      }
    >
      <div className={s.stack}>
        {phase.kind === 'IDLE' && (
          <p className={s.hint}>
            하루에 한 번 저절로 확인하고, [업데이트 확인] 을 누르면 바로 알아봅니다. 확인만 하고
            설치하지는 않으므로 직접 [업데이트 시작] 을 누르기 전에는 바뀌지 않습니다.
            인터넷에 연결되어 있지 않아도 학생명단 기능은 모두 그대로 쓸 수 있습니다.
          </p>
        )}

        {phase.kind === 'LATEST' && (
          <Notice tone="info">
            최신 버전을 사용하고 있습니다{info.data && ` (v${info.data.appVersion})`}.
          </Notice>
        )}

        {phase.kind === 'FOUND' && (
          <>
            <Notice tone="info">
              <strong>새 버전 {phase.update.version}</strong> 을 사용할 수 있습니다.
              {info.data && <span className={s.sub}> 지금 v{info.data.appVersion}</span>}
            </Notice>
            {phase.update.body && <pre className={s.notes}>{phase.update.body}</pre>}
            <p className={s.hint}>
              [업데이트] 를 누르면 내려받아 설치하고 <strong>프로그램이 저절로 다시 시작됩니다.</strong>{' '}
              학생 자료는 프로그램과 따로 보관되므로 그대로 남습니다.
            </p>
            <div className={s.row}>
              <Button variant="primary" onClick={() => run.mutate(phase.update)}>
                업데이트
              </Button>
              <Button variant="ghost" onClick={() => setPhase({ kind: 'IDLE' })}>
                나중에
              </Button>
            </div>
          </>
        )}

        {phase.kind === 'DOWNLOADING' && (
          <>
            <p className={s.stepText}>업데이트 파일을 내려받고 있습니다.</p>
            <div className={s.bar}>
              <div
                className={s.barFill}
                style={{
                  width: phase.total
                    ? `${Math.min(100, Math.round((phase.got / phase.total) * 100))}%`
                    : '35%',
                }}
              />
            </div>
            <p className={s.hint}>
              {phase.total
                ? `${Math.round((phase.got / phase.total) * 100)}% (${kb(phase.got)} / ${kb(
                    phase.total,
                  )} KB)`
                : `${kb(phase.got)} KB`}
              {' · '}창을 닫지 말아 주세요.
            </p>
          </>
        )}

        {phase.kind === 'INSTALLING' && (
          <Notice tone="warn">
            <strong>프로그램이 잠시 뒤 저절로 닫혔다가 다시 켜집니다.</strong>
            <div className={s.warnBody}>
              설치가 끝나면 새 버전으로 열립니다. 그동안 아무것도 누르지 말아 주세요.
              혹시 다시 켜지지 않으면 시작 메뉴에서 프로그램을 다시 실행해 주세요.
            </div>
          </Notice>
        )}

        {phase.kind === 'FAILED' && (
          <Notice tone="error" detail={detail}>
            {phase.message}
          </Notice>
        )}
      </div>
    </Card>
  )
}
