import { useQuery } from '@tanstack/react-query'
import { HashRouter, Navigate, Route, Routes } from 'react-router-dom'

import { AppShell } from '@/components/AppShell'
import { appApi } from '@/ipc/app'
import { errorDetail, errorMessage } from '@/ipc/invoke'
import { issueApi } from '@/ipc/issue'
import { settingsApi } from '@/ipc/settings'
import { AddressRulesPage } from '@/pages/AddressRulesPage'
import { ExportPage } from '@/pages/ExportPage'
import { GraduatesPage } from '@/pages/GraduatesPage'
import { ImportPage } from '@/pages/ImportPage'
import { IssuesPage } from '@/pages/IssuesPage'
import { StatsPage } from '@/pages/StatsPage'
import { TransferInPage } from '@/pages/TransferInPage'
import { TransferOutPage } from '@/pages/TransferOutPage'
import { YearTransitionPage } from '@/pages/YearTransitionPage'
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
          <Route path="/year-transition" element={<YearTransitionPage />} />
          <Route path="/graduates" element={<GraduatesPage />} />
          <Route path="/export" element={<ExportPage />} />
          <Route path="/settings" element={<SettingsPage />} />
        </Route>

        <Route path="*" element={<Navigate to={ready ? '/students' : '/welcome'} replace />} />
      </Routes>
    </HashRouter>
  )
}
