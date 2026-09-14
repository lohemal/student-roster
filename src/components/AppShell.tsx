import { NavLink, Outlet } from 'react-router-dom'
import {
  ArrowRightLeft,
  BarChart3,
  CalendarRange,
  ClipboardCheck,
  FileSpreadsheet,
  GraduationCap,
  LogIn,
  LogOut,
  MapPinned,
  Settings,
  Users,
  type LucideIcon,
} from 'lucide-react'

import { yearLabel } from '@/lib/schoolYear'
import s from './AppShell.module.css'

interface MenuItem {
  to: string
  label: string
  icon: LucideIcon
  /** 사이드바에 보여줄 개수 (확인 필요 등) */
  count?: number
}

interface MenuGroup {
  label: string
  items: MenuItem[]
}

export function buildMenu(issueCount: number): MenuGroup[] {
  return [
    {
      label: '학생',
      items: [
        { to: '/students', label: '학생명단', icon: Users },
        { to: '/transfer-in', label: '전입생', icon: LogIn },
        { to: '/transfer-out', label: '전출생', icon: LogOut },
        { to: '/issues', label: '확인 필요', icon: ClipboardCheck, count: issueCount },
      ],
    },
    {
      label: '관리',
      items: [
        { to: '/address-rules', label: '주소 규칙', icon: MapPinned },
        { to: '/stats', label: '통계', icon: BarChart3 },
        { to: '/year-transition', label: '학년도 전환', icon: ArrowRightLeft },
        { to: '/graduates', label: '졸업생', icon: GraduationCap },
      ],
    },
    {
      label: '자료',
      items: [
        { to: '/export', label: '파일 내보내기', icon: FileSpreadsheet },
        { to: '/settings', label: '설정', icon: Settings },
      ],
    },
  ]
}

interface Props {
  schoolName?: string
  currentYear?: number | null
  appVersion?: string
  issueCount?: number
}

export function AppShell({ schoolName, currentYear, appVersion, issueCount = 0 }: Props) {
  const menu = buildMenu(issueCount)
  return (
    <div className={s.shell}>
      <aside className={s.sidebar}>
        <div className={s.brand}>
          <div className={s.brandMark} aria-hidden="true">
            <CalendarRange size={20} strokeWidth={2.4} />
          </div>
          <div className={s.brandText}>
            <span className={s.brandTitle}>학생명단 관리</span>
            <span className={s.brandSub}>{schoolName || '학교 미설정'}</span>
          </div>
        </div>

        <nav className={s.nav}>
          {menu.map((g) => (
            <div key={g.label} className={s.group}>
              <div className={s.groupLabel}>{g.label}</div>
              {g.items.map((m) => (
                <NavLink
                  key={m.to}
                  to={m.to}
                  className={({ isActive }) =>
                    isActive ? `${s.navItem} ${s.navItemActive}` : s.navItem
                  }
                >
                  <m.icon size={18} strokeWidth={2} />
                  <span className={s.navLabel}>{m.label}</span>
                  {m.count ? <span className={s.navCount}>{m.count > 99 ? '99+' : m.count}</span> : null}
                </NavLink>
              ))}
            </div>
          ))}
        </nav>

        <div className={s.sidebarFoot}>
          <span className={s.yearChip}>{yearLabel(currentYear)}</span>
          <span className={s.version}>v{appVersion ?? '0.0.0'}</span>
        </div>
      </aside>

      <main className={s.main}>
        <Outlet />
      </main>
    </div>
  )
}
