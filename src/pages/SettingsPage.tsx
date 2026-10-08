import { useEffect, useRef, useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { Plus, RefreshCw, Trash2, Users } from 'lucide-react'

import {
  Badge,
  Button,
  Card,
  ErrorNotice,
  Field,
  FieldAction,
  Input,
  Notice,
  Page,
} from '@/components/ui'
import { DataCard } from '@/features/system/DataCard'
import { UpdateCard } from '@/features/system/UpdateCard'
import { YearDeleteDialog } from '@/features/system/YearDelete'
import { watchJob, type JobProgress } from '@/ipc/import'
import { issueApi } from '@/ipc/issue'
import { settingsApi } from '@/ipc/settings'
import { siblingApi, type ScanResult } from '@/ipc/sibling'
import { guessSchoolYear, yearLabel } from '@/lib/schoolYear'
import s from './SettingsPage.module.css'

export function SettingsPage() {
  const qc = useQueryClient()
  const settings = useQuery({ queryKey: ['settings'], queryFn: settingsApi.get })

  const invalidate = () => {
    qc.invalidateQueries({ queryKey: ['settings'] })
    qc.invalidateQueries({ queryKey: ['app-info'] })
  }

  // ---- 학교 이름 ----
  const [schoolName, setSchoolName] = useState('')
  const [savedAt, setSavedAt] = useState<number | null>(null)
  useEffect(() => {
    if (settings.data) setSchoolName(settings.data.schoolName)
  }, [settings.data])

  const saveSchool = useMutation({
    mutationFn: () => settingsApi.saveSchool(schoolName),
    onSuccess: () => {
      setSavedAt(Date.now())
      invalidate()
    },
  })

  // ---- 학년도 ----
  const [newYear, setNewYear] = useState<string>(String(guessSchoolYear()))
  const createYear = useMutation({
    mutationFn: (year: number) => settingsApi.createYear(year, !settings.data?.currentYear),
    onSuccess: invalidate,
  })
  const setCurrent = useMutation({
    mutationFn: (year: number) => settingsApi.setCurrentYear(year),
    onSuccess: invalidate,
  })

  // 지우기는 확인 창을 먼저 연다 — 버튼 한 번으로 사라지면 안 된다
  const [deleting, setDeleting] = useState<number | null>(null)
  const [yearDone, setYearDone] = useState<string | null>(null)

  // ---- 자료 점검 ----
  const recompute = useMutation({
    mutationFn: () => issueApi.recompute(settings.data!.currentYear!),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['issue-summary'] })
      qc.invalidateQueries({ queryKey: ['issues'] })
      qc.invalidateQueries({ queryKey: ['students'] })
    },
  })

  // ---- 형제 후보 다시 찾기 ----
  const [scanProgress, setScanProgress] = useState<JobProgress | null>(null)
  const [scanResult, setScanResult] = useState<ScanResult | null>(null)
  const [scanError, setScanError] = useState<unknown>(null)
  const [scanning, setScanning] = useState(false)
  const stopWatch = useRef<(() => void) | null>(null)
  useEffect(() => () => stopWatch.current?.(), [])

  const rescan = useMutation({
    mutationFn: async () => {
      setScanError(null)
      setScanResult(null)
      setScanProgress(null)
      setScanning(true)
      const started = await siblingApi.rescan(settings.data!.currentYear!)
      stopWatch.current?.()
      stopWatch.current = watchJob<ScanResult>(started.jobId, {
        onProgress: setScanProgress,
        onDone: (r) => {
          setScanResult(r)
          setScanning(false)
          qc.invalidateQueries({ queryKey: ['students'] })
          qc.invalidateQueries({ queryKey: ['siblings'] })
          qc.invalidateQueries({ queryKey: ['issue-summary'] })
          qc.invalidateQueries({ queryKey: ['issues'] })
        },
        onError: (e) => {
          setScanError(e)
          setScanning(false)
        },
      })
    },
    onError: (e) => {
      setScanError(e)
      setScanning(false)
    },
  })

  const scanPct =
    scanProgress && scanProgress.total > 0
      ? Math.round((scanProgress.done / scanProgress.total) * 100)
      : 0

  const data = settings.data

  return (
    <Page title="설정" year={yearLabel(data?.currentYear)}>
      <ErrorNotice error={settings.error} />

      <div className={s.grid}>
        <Card title="학교" description="사이드바와 내보내기 파일 제목에 쓰입니다.">
          <div className={s.stack}>
            <div className={s.row}>
              <Field label="학교 이름">
                <Input
                  value={schoolName}
                  onChange={(e) => setSchoolName(e.target.value)}
                  placeholder="예: ○○초등학교"
                  maxLength={50}
                />
              </Field>
              <Button
                variant="primary"
                onClick={() => saveSchool.mutate()}
                disabled={saveSchool.isPending || !schoolName.trim()}
              >
                저장
              </Button>
            </div>
            {saveSchool.isSuccess && savedAt && <span className={s.saved}>저장했습니다.</span>}
            <ErrorNotice error={saveSchool.error} />
          </div>
        </Card>

        <Card
          title="학년도"
          description="현재 학년도의 학생이 학생명단 기본 화면에 나옵니다. 이전 학년도 자료는 지워지지 않습니다."
        >
          <div className={s.stack}>
            {data && data.years.length === 0 && (
              <Notice tone="info">아직 학년도가 없습니다. 아래에서 첫 학년도를 만들어 주세요.</Notice>
            )}

            <div className={s.yearList}>
              {data?.years.map((y) => (
                <div
                  key={y.year}
                  className={y.isCurrent ? `${s.yearRow} ${s.yearRowCurrent}` : s.yearRow}
                >
                  <span className={s.yearName}>{y.year}학년도</span>
                  <span className={s.yearMeta}>재학생 {y.studentCount.toLocaleString()}명</span>
                  {y.isCurrent ? (
                    <Badge tone="info">현재 학년도</Badge>
                  ) : (
                    <>
                      <Button
                        size="sm"
                        variant="outline"
                        onClick={() => setCurrent.mutate(y.year)}
                        disabled={setCurrent.isPending}
                      >
                        현재로 지정
                      </Button>
                      {/*
                        지울 수 있는지는 Rust 가 가린다. 여기서 달력 연도로 미리
                        감추면 1~2월에 규칙이 어긋난다 — 누르면 까닭을 알려 준다.
                      */}
                      <Button
                        size="sm"
                        variant="ghost"
                        icon={Trash2}
                        onClick={() => setDeleting(y.year)}
                        aria-label={`${y.year}학년도 삭제`}
                      >
                        삭제
                      </Button>
                    </>
                  )}
                </div>
              ))}
            </div>

            <div className={s.row}>
              <Field label="새 학년도" hint="3월에 시작하는 연도를 씁니다. 1~2월은 이전 학년도입니다.">
                <Input
                  type="number"
                  min={2000}
                  max={2100}
                  value={newYear}
                  onChange={(e) => setNewYear(e.target.value)}
                />
              </Field>
              <FieldAction>
                <Button
                  icon={Plus}
                  onClick={() => createYear.mutate(Number(newYear))}
                  disabled={createYear.isPending || !/^\d{4}$/.test(newYear)}
                >
                  추가
                </Button>
              </FieldAction>
            </div>
            {yearDone && <Notice tone="success">{yearDone}</Notice>}
            <ErrorNotice error={createYear.error ?? setCurrent.error} />
          </div>
        </Card>

        <Card
          title="자료 점검"
          description="자료를 밖에서 고쳤거나 예전 명단을 들여온 뒤에 한 번 눌러 주세요."
        >
          <div className={s.stack}>
            <Notice tone="info">
              학생을 한 명씩 저장할 때는 확인 필요가 저절로 다시 계산됩니다. 이 버튼은 그 계산을
              현재 학년도 학생 전체에 대해 한 번에 돌립니다.
            </Notice>
            {recompute.isSuccess && (
              <span className={s.saved}>
                {recompute.data.toLocaleString('ko-KR')}명을 다시 살펴봤습니다.
              </span>
            )}
            <ErrorNotice error={recompute.error} />
            <div>
              <Button
                icon={RefreshCw}
                variant="outline"
                onClick={() => recompute.mutate()}
                disabled={recompute.isPending || data?.currentYear == null}
              >
                확인 필요 다시 계산
              </Button>
            </div>
          </div>
        </Card>

        <Card
          title="본교 형제 찾기"
          description="보호자 성명·연락처 가운데 값이 있는 항목이 두 가지 이상 같은 학생을 형제 후보로 찾습니다."
        >
          <div className={s.stack}>
            <Notice tone="info">
              이미 <b>형제로 확인</b>했거나 <b>형제 아님</b>으로 정한 관계는 그대로 둡니다. 사용자가
              내린 판단을 다시 묻지 않습니다.
            </Notice>

            {scanning && (
              <>
                <div className={s.row}>
                  <span className={s.progressText}>
                    {scanProgress?.stageLabel ?? '시작하는 중'}
                  </span>
                  <span className={s.progressText}>{scanPct}%</span>
                </div>
                <div className={s.bar}>
                  <div className={s.barFill} style={{ width: `${scanPct}%` }} />
                </div>
              </>
            )}

            {scanResult && !scanning && (
              <span className={s.saved}>
                재학생 {scanResult.scanned.toLocaleString()}명을 살펴봐 새 형제 후보{' '}
                {scanResult.newCandidates.toLocaleString()}쌍을 찾았습니다. 이미 확인한 관계{' '}
                {scanResult.confirmed.toLocaleString()}쌍은 그대로 두었습니다.
              </span>
            )}
            {scanResult && !scanning && scanResult.skippedValues > 0 && (
              <Notice tone="warn">
                같은 값을 여러 학생이 나눠 쓰고 있는 항목 {scanResult.skippedValues}가지는 사람을
                가려내지 못해 후보를 만드는 데 쓰지 않았습니다. 학교 대표번호처럼 공용으로 적어 둔
                연락처가 있는지 확인해 주세요.
              </Notice>
            )}

            <ErrorNotice error={scanError} />
            <div>
              <Button
                icon={Users}
                variant="outline"
                onClick={() => rescan.mutate()}
                disabled={scanning || data?.currentYear == null}
              >
                {scanning ? '찾는 중…' : '형제 후보 다시 찾기'}
              </Button>
            </div>
          </div>
        </Card>

        <DataCard />

        {deleting != null && (
          <YearDeleteDialog
            year={deleting}
            onClose={() => setDeleting(null)}
            onDone={(message) => {
              setDeleting(null)
              setYearDone(message)
              invalidate()
              // 학년도가 사라지면 명단 · 통계 · 내보내기의 고르기도 함께 바뀐다
              qc.invalidateQueries({ queryKey: ['students'] })
              qc.invalidateQueries({ queryKey: ['stats'] })
              qc.invalidateQueries({ queryKey: ['class-options'] })
              qc.invalidateQueries({ queryKey: ['issue-summary'] })
              qc.invalidateQueries({ queryKey: ['backups'] })
            }}
          />
        )}

        <UpdateCard />

      </div>
    </Page>
  )
}
