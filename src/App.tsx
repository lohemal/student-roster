import { useQuery } from '@tanstack/react-query'
import { HashRouter, Navigate, Route, Routes } from 'react-router-dom'

import { AppShell } from '@/components/AppShell'
import { appApi } from '@/ipc/app'
import { errorDetail, errorMessage } from '@/ipc/invoke'
import { issueApi } from '@/ipc/issue'
import { settingsApi } from '@/ipc/settings'
import { PlaceholderPage } from '@/pages/PlaceholderPage'
import { AddressRulesPage } from '@/pages/AddressRulesPage'
import { ImportPage } from '@/pages/ImportPage'
import { IssuesPage } from '@/pages/IssuesPage'
import { StatsPage } from '@/pages/StatsPage'
import { TransferInPage } from '@/pages/TransferInPage'
import { TransferOutPage } from '@/pages/TransferOutPage'
import { SettingsPage } from '@/pages/SettingsPage'
import { StudentsPage } from '@/pages/StudentsPage'
import { WelcomePage } from '@/pages/WelcomePage'
import s from './App.module.css'

export function App() {
  const info = useQuery({ queryKey: ['app-info'], queryFn: appApi.info })
  const settings = useQuery({ queryKey: ['settings'], queryFn: settingsApi.get })
  const year = settings.data?.currentYear ?? null
  const issues = useQuery({
    queryKey: ['issue-summary', year],
    queryFn: () => issueApi.summary(year!),
    enabled: year != null,
  })

  if (info.isLoading) {
    return (
      <div className={s.splash}>
        <div className={s.splashMark} />
        <p className={s.splashText}>학생명단 관리 시스템을 준비하는 중…</p>
      </div>
    )
  }

  if (info.error || !info.data) {
    return (
      <div className={s.splash}>
        <h1 className={s.fatalTitle}>프로그램을 시작하지 못했습니다</h1>
        <p className={s.fatalMsg}>{errorMessage(info.error)}</p>
        {errorDetail(info.error) && (
          <details className={s.fatalDetail}>
            <summary>자세히</summary>
            <pre className="selectable">{errorDetail(info.error)}</pre>
          </details>
        )}
      </div>
    )
  }

  const ready = info.data.setupCompleted

  return (
    <HashRouter>
      <Routes>
        {/* 첫 실행 화면은 사이드바 없이 단독으로 */}
        <Route path="/welcome" element={<WelcomePage />} />

        <Route
          element={
            ready ? (
              <AppShell
                appVersion={info.data.appVersion}
                schoolName={settings.data?.schoolName}
                currentYear={settings.data?.currentYear}
                issueCount={issues.data?.total ?? 0}
              />
            ) : (
              <Navigate to="/welcome" replace />
            )
          }
        >
          <Route path="/students" element={<StudentsPage />} />
          <Route path="/students/import" element={<ImportPage />} />
          <Route path="/transfer-in" element={<TransferInPage />} />
          <Route path="/transfer-out" element={<TransferOutPage />} />
          <Route path="/issues" element={<IssuesPage />} />
          <Route path="/address-rules" element={<AddressRulesPage />} />
          <Route path="/stats" element={<StatsPage />} />
          <Route
            path="/year-transition"
            element={
              <PlaceholderPage
                title="학년도 전환"
                phase={9}
                description="반배정·신입생 자료로 다음 학년도를 미리 보고 생성합니다."
              />
            }
          />
          <Route
            path="/graduates"
            element={
              <PlaceholderPage
                title="졸업생"
                phase={9}
                description="학년도별 졸업생을 조회하고 내보냅니다."
              />
            }
          />
          <Route
            path="/export"
            element={
              <PlaceholderPage
                title="파일 내보내기"
                phase={8}
                description="원하는 항목만 고른 Excel 명단과 외부 시스템 업로드 양식을 만듭니다."
              />
            }
          />
          <Route path="/settings" element={<SettingsPage />} />
        </Route>

        <Route path="*" element={<Navigate to={ready ? '/students' : '/welcome'} replace />} />
      </Routes>
    </HashRouter>
  )
}
