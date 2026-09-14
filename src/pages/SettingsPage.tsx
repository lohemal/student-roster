import { useEffect, useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { FolderOpen, Plus } from 'lucide-react'

import { Badge, Button, Card, ErrorNotice, Field, Input, Notice, Page } from '@/components/ui'
import { appApi } from '@/ipc/app'
import { settingsApi } from '@/ipc/settings'
import { guessSchoolYear, yearLabel } from '@/lib/schoolYear'
import s from './SettingsPage.module.css'

export function SettingsPage() {
  const qc = useQueryClient()
  const settings = useQuery({ queryKey: ['settings'], queryFn: settingsApi.get })
  const info = useQuery({ queryKey: ['app-info'], queryFn: appApi.info })

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
                  placeholder="예: 한솔초등학교"
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
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => setCurrent.mutate(y.year)}
                      disabled={setCurrent.isPending}
                    >
                      현재로 지정
                    </Button>
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
              <Button
                icon={Plus}
                onClick={() => createYear.mutate(Number(newYear))}
                disabled={createYear.isPending || !/^\d{4}$/.test(newYear)}
              >
                추가
              </Button>
            </div>
            <ErrorNotice error={createYear.error ?? setCurrent.error} />
          </div>
        </Card>

        <Card title="자료 위치" description="학생 자료는 이 컴퓨터 안에만 저장됩니다. 외부로 전송하지 않습니다.">
          <div className={s.stack}>
            <dl className={s.kv}>
              <dt>자료 파일</dt>
              <dd className="selectable">{info.data?.dbPath ?? '…'}</dd>
              <dt>프로그램 버전</dt>
              <dd>v{info.data?.appVersion ?? '…'}</dd>
              <dt>자료 구조 버전</dt>
              <dd>
                {info.data ? `v${info.data.schemaVersion}` : '…'}
              </dd>
            </dl>
            <Notice tone="info">
              개인정보 보호를 위해 이 PC의 Windows 계정에 암호를 걸고, 가능하면 BitLocker(드라이브 암호화)를 켜 두세요.
              백업·복원 기능은 Phase 10에서 추가됩니다.
            </Notice>
            <div>
              <Button icon={FolderOpen} variant="outline" onClick={() => appApi.openDataFolder()}>
                자료 폴더 열기
              </Button>
            </div>
          </div>
        </Card>
      </div>
    </Page>
  )
}
