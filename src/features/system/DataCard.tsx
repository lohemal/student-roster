import { useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { open } from '@tauri-apps/plugin-dialog'
import {
  AlertTriangle,
  Check,
  FolderOpen,
  HardDriveDownload,
  List,
  Power,
  RotateCcw,
  Save,
} from 'lucide-react'

import { Button, Card, ErrorNotice, Notice, TableWrap, tableClass } from '@/components/ui'
import { appApi } from '@/ipc/app'
import { backupApi, fileSize, type BackupEntry } from '@/ipc/backup'
import s from './system.module.css'

const n = (v: number) => v.toLocaleString('ko-KR')

/**
 * 자료 관리 — 백업과 복원.
 *
 * 학생 자료는 이 컴퓨터 안에만 있다. 그래서 **되돌릴 수 있는 것**이 마지막 안전장치다.
 * 복원은 평범한 버튼과 눈에 띄게 구분해 두었다 — 지금 자료를 통째로 바꾸는 일이므로.
 */
export function DataCard() {
  const qc = useQueryClient()
  const info = useQuery({ queryKey: ['app-info'], queryFn: appApi.info })
  const status = useQuery({ queryKey: ['backup-status'], queryFn: backupApi.status })
  const [showList, setShowList] = useState(false)
  const [error, setError] = useState<unknown>(null)
  const [madeAt, setMadeAt] = useState<string | null>(null)
  /** 되돌리기 확인을 기다리는 파일 */
  const [asking, setAsking] = useState<BackupEntry | null>(null)
  /** 되돌릴 준비가 끝나 다시 켜기를 기다리는 중 */
  const [staged, setStaged] = useState<string | null>(null)

  const refresh = () => {
    qc.invalidateQueries({ queryKey: ['backup-status'] })
    qc.invalidateQueries({ queryKey: ['backup-list'] })
  }

  const list = useQuery({
    queryKey: ['backup-list'],
    queryFn: backupApi.list,
    enabled: showList,
  })

  const backupNow = useMutation({
    mutationFn: backupApi.now,
    onSuccess: (e) => {
      setError(null)
      setMadeAt(e.createdAt)
      refresh()
    },
    onError: setError,
  })

  const stage = useMutation({
    mutationFn: (path: string) => backupApi.stageRestore(path),
    onSuccess: (r) => {
      setError(null)
      setAsking(null)
      setStaged(r.fileName)
      refresh()
    },
    onError: (e) => {
      setAsking(null)
      setError(e)
    },
  })

  const cancelStaged = useMutation({
    mutationFn: backupApi.cancelRestore,
    onSuccess: () => {
      setStaged(null)
      refresh()
    },
    onError: setError,
  })

  const remove = useMutation({
    mutationFn: (fileName: string) => backupApi.remove(fileName),
    onSuccess: refresh,
    onError: setError,
  })

  /** 백업 폴더 밖에 보관해 둔 파일 고르기 */
  const pickOutside = async () => {
    setError(null)
    const path = await open({
      multiple: false,
      filters: [{ name: '자료 파일', extensions: ['db'] }],
    })
    if (!path || typeof path !== 'string') return // 취소는 오류가 아니다
    try {
      const entry = await backupApi.checkFile(path)
      if (!entry.ok) {
        setError({ code: 'INVALID_INPUT', userMessage: entry.problem ?? '되돌릴 수 없는 파일입니다.' })
        return
      }
      setAsking(entry)
    } catch (e) {
      setError(e)
    }
  }

  const waiting = staged != null || status.data?.restoreWaiting

  return (
    <>
      <Card
        title="자료 위치"
        description="학생 자료는 이 컴퓨터 안에만 저장됩니다. 외부로 전송하지 않습니다."
      >
        <div className={s.stack}>
          <dl className={s.kv}>
            <dt>자료 파일</dt>
            <dd className="selectable">{info.data?.dbPath ?? '…'}</dd>
            <dt>백업 폴더</dt>
            <dd className="selectable">{info.data?.backupDir ?? '…'}</dd>
            <dt>프로그램 버전</dt>
            <dd>v{info.data?.appVersion ?? '…'}</dd>
            <dt>자료 구조 버전</dt>
            <dd>{info.data ? `v${info.data.schemaVersion}` : '…'}</dd>
          </dl>
          <Notice tone="info">
            프로그램을 지우거나 새로 설치해도 이 자료는 그대로 남습니다. 자료를 없애려면 위
            폴더를 직접 지워야 합니다. 개인정보 보호를 위해 이 PC의 Windows 계정에 암호를 걸고,
            가능하면 BitLocker(드라이브 암호화)를 켜 두세요.
          </Notice>
          <div className={s.row}>
            <Button icon={FolderOpen} variant="outline" onClick={() => appApi.openDataFolder()}>
              자료 폴더 열기
            </Button>
          </div>
        </div>
      </Card>

      <Card
        className={showList ? s.wide : undefined}
        title="백업"
        description="하루에 한 번 자동으로 만들고, 필요할 때 직접 만들 수 있습니다."
      >
        <div className={s.stack}>
          <ErrorNotice error={error} />

          <dl className={s.kv}>
            <dt>최근 백업</dt>
            <dd>
              {status.data?.latest
                ? `${status.data.latest.createdAt} · ${status.data.latest.kindLabel}`
                : '아직 없습니다'}
            </dd>
            <dt>백업 파일</dt>
            <dd>
              {status.data ? `${n(status.data.total)}개` : '…'}
              {status.data && status.data.broken > 0 && (
                <span className={s.bad}> · 확인 실패 {n(status.data.broken)}개</span>
              )}
            </dd>
          </dl>

          {madeAt && (
            <Notice tone="success">
              백업을 만들고 확인까지 마쳤습니다. ({madeAt})
            </Notice>
          )}

          <Notice tone="warn">
            <strong>백업 파일에는 학생 개인정보가 그대로 들어 있습니다.</strong> USB나 다른
            컴퓨터에 옮겨 둘 때는 그 저장장치의 보관에 주의해 주세요.
          </Notice>

          <div className={s.row}>
            <Button
              variant="primary"
              icon={Save}
              disabled={backupNow.isPending}
              onClick={() => backupNow.mutate()}
            >
              {backupNow.isPending ? '만드는 중…' : '지금 백업'}
            </Button>
            <Button icon={List} onClick={() => setShowList((v) => !v)}>
              {showList ? '백업 목록 닫기' : '백업 관리'}
            </Button>
            <Button icon={FolderOpen} variant="outline" onClick={() => backupApi.openFolder()}>
              백업 폴더 열기
            </Button>
          </div>

          {showList && (
            <>
              <p className={s.hint}>
                자동 백업은 최근 {n(status.data?.keptAuto ?? 30)}개까지 두고 오래된 것부터
                정리합니다. 수동 백업과 학년도 전환 전·복원 전 백업은 정리하지 않습니다.
              </p>
              <TableWrap>
                <thead>
                  <tr>
                    <th>만든 때</th>
                    <th>종류</th>
                    <th className={tableClass.num}>크기</th>
                    <th className={tableClass.num}>학생</th>
                    <th>확인</th>
                    <th />
                  </tr>
                </thead>
                <tbody>
                  {(list.data ?? []).map((e) => (
                    <tr key={e.fileName}>
                      <td className="selectable">{e.createdAt}</td>
                      <td>{e.kindLabel}</td>
                      <td className={tableClass.num}>{fileSize(e.size)}</td>
                      <td className={tableClass.num}>
                        {e.students != null ? `${n(e.students)}명` : '—'}
                      </td>
                      <td>
                        {e.ok ? (
                          <span className={s.good}>
                            <Check size={13} /> 정상
                          </span>
                        ) : (
                          <span className={s.bad} title={e.problem ?? undefined}>
                            <AlertTriangle size={13} /> 확인 실패
                          </span>
                        )}
                      </td>
                      <td className={s.rowActions}>
                        <Button
                          size="sm"
                          variant="outline"
                          icon={RotateCcw}
                          disabled={!e.ok || waiting}
                          onClick={() => setAsking(e)}
                        >
                          이 백업으로 되돌리기
                        </Button>
                        <Button
                          size="sm"
                          variant="ghost"
                          disabled={waiting}
                          onClick={() => remove.mutate(e.fileName)}
                        >
                          지우기
                        </Button>
                      </td>
                    </tr>
                  ))}
                  {(list.data ?? []).length === 0 && (
                    <tr>
                      <td colSpan={6} className={tableClass.muted}>
                        아직 백업이 없습니다.
                      </td>
                    </tr>
                  )}
                </tbody>
              </TableWrap>
            </>
          )}
        </div>
      </Card>

      {/* 복원 — 지금 자료를 통째로 바꾸는 일이라 따로 둔다 */}
      <Card className={`${s.danger} ${s.wide}`} title="백업에서 복원">
        <div className={s.stack}>
          <p className={s.hint}>
            고른 백업의 자료로 <strong>지금 자료를 통째로 바꿉니다.</strong> 되돌리기 직전에
            지금 자료를 <code>before_restore_…</code> 로 한 번 더 백업하므로, 고른 백업이 생각과
            달랐다면 그 파일로 다시 돌아올 수 있습니다.
          </p>

          {waiting ? (
            <Notice tone="warn">
              <strong>되돌릴 준비가 끝났습니다{staged ? ` (${staged})` : ''}.</strong>
              <div className={s.warnBody}>
                지금 자료는 아직 그대로입니다. 프로그램을 끝내고 다시 열면 그때 자료를 바꿉니다 —
                자료 파일을 쓰는 중에 바꿔치기하지 않으려는 것입니다.
              </div>
              <div className={s.row}>
                <Button variant="danger" icon={Power} onClick={() => backupApi.quit()}>
                  프로그램 종료
                </Button>
                <Button variant="ghost" onClick={() => cancelStaged.mutate()}>
                  되돌리기 취소
                </Button>
              </div>
            </Notice>
          ) : (
            <div className={s.row}>
              <Button icon={List} onClick={() => setShowList(true)}>
                백업 목록에서 고르기
              </Button>
              <Button icon={HardDriveDownload} variant="outline" onClick={pickOutside}>
                다른 곳에 보관한 파일에서
              </Button>
            </div>
          )}

          {asking && (
            <Notice tone="error">
              <strong>
                {asking.createdAt} {asking.kindLabel}
                {asking.students != null && ` · 학생 ${n(asking.students)}명`}
              </strong>
              <div className={s.warnBody}>
                이 자료로 되돌립니다. 그 뒤에 넣은 학생·전입·전출·졸업 기록은 이 백업에 없다면
                사라집니다. 계속할까요?
              </div>
              <div className={s.row}>
                <Button
                  variant="danger"
                  disabled={stage.isPending}
                  onClick={() => stage.mutate(asking.path)}
                >
                  {stage.isPending ? '준비하는 중…' : '되돌리기'}
                </Button>
                <Button variant="ghost" onClick={() => setAsking(null)}>
                  그만두기
                </Button>
              </div>
            </Notice>
          )}
        </div>
      </Card>
    </>
  )
}
