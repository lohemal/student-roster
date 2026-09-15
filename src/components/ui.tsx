import { useEffect } from 'react'
import type { ButtonHTMLAttributes, InputHTMLAttributes, ReactNode, SelectHTMLAttributes, TextareaHTMLAttributes } from 'react'
import { AlertTriangle, CheckCircle2, Info, X, XCircle, type LucideIcon } from 'lucide-react'

import { errorDetail, errorMessage } from '@/ipc/invoke'
import s from './ui.module.css'

function cx(...parts: Array<string | false | null | undefined>) {
  return parts.filter(Boolean).join(' ')
}

/* ---------- Page ---------- */

export function Page({
  title,
  description,
  year,
  actions,
  children,
}: {
  title: string
  description?: string
  /** 상단 바에 보여줄 학년도 표시. 생략하면 표시하지 않는다 */
  year?: ReactNode
  actions?: ReactNode
  children: ReactNode
}) {
  return (
    <div className={s.page}>
      <header className={s.topbar}>
        <h1 className={s.topbarTitle}>{title}</h1>
        {year && <span className={s.yearChip}>{year}</span>}
        {description && <span className={s.topbarDesc}>{description}</span>}
        <span className={s.topbarSpacer} />
        {actions && <div className={s.topbarActions}>{actions}</div>}
      </header>
      <div className={s.body}>{children}</div>
    </div>
  )
}

/* ---------- Card ---------- */

export function Card({
  title,
  description,
  actions,
  children,
  className,
}: {
  title?: string
  description?: string
  actions?: ReactNode
  children: ReactNode
  className?: string
}) {
  return (
    <section className={cx(s.card, className)}>
      {(title || actions) && (
        <header className={s.cardHead}>
          <div>
            {title && <h2 className={s.cardTitle}>{title}</h2>}
            {description && <p className={s.cardDesc}>{description}</p>}
          </div>
          {actions}
        </header>
      )}
      <div className={s.cardBody}>{children}</div>
    </section>
  )
}

/* ---------- Button ---------- */

type Variant = 'primary' | 'secondary' | 'outline' | 'ghost' | 'danger'

export function Button({
  variant = 'secondary',
  size,
  icon: IconCmp,
  children,
  className,
  ...rest
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: Variant
  size?: 'sm'
  icon?: LucideIcon
}) {
  return (
    <button
      type="button"
      className={cx(s.btn, s[variant], size === 'sm' && s.btnSm, className)}
      {...rest}
    >
      {IconCmp && <IconCmp size={size === 'sm' ? 14 : 16} strokeWidth={2} />}
      {children}
    </button>
  )
}

/* ---------- Form ---------- */

export function Field({
  label,
  hint,
  className,
  children,
}: {
  label: string
  hint?: ReactNode
  /** 격자 안에서 칸을 더 차지해야 할 때 (예: 주소) */
  className?: string
  children: ReactNode
}) {
  return (
    <label className={cx(s.field, className)}>
      <span className={s.label}>{label}</span>
      {children}
      {hint && <span className={s.hint}>{hint}</span>}
    </label>
  )
}

export function Input({ className, ...rest }: InputHTMLAttributes<HTMLInputElement>) {
  return <input className={cx(s.input, className)} {...rest} />
}

export function Select({ className, ...rest }: SelectHTMLAttributes<HTMLSelectElement>) {
  return <select className={cx(s.select, className)} {...rest} />
}

export function Textarea({ className, ...rest }: TextareaHTMLAttributes<HTMLTextAreaElement>) {
  return <textarea className={cx(s.input, s.textarea, className)} {...rest} />
}

/* ---------- Badge ---------- */

export type Tone = 'success' | 'info' | 'warn' | 'error' | 'neutral'

const BADGE: Record<Tone, string> = {
  success: s.badgeSuccess,
  info: s.badgeInfo,
  warn: s.badgeWarn,
  error: s.badgeError,
  neutral: s.badgeNeutral,
}

export function Badge({ tone = 'neutral', children }: { tone?: Tone; children: ReactNode }) {
  return <span className={cx(s.badge, BADGE[tone])}>{children}</span>
}

/* ---------- Notice ---------- */

const NOTICE: Record<Exclude<Tone, 'neutral'>, { cls: string; icon: LucideIcon }> = {
  info: { cls: s.noticeInfo, icon: Info },
  warn: { cls: s.noticeWarn, icon: AlertTriangle },
  error: { cls: s.noticeError, icon: XCircle },
  success: { cls: s.noticeSuccess, icon: CheckCircle2 },
}

export function Notice({
  tone = 'info',
  children,
  detail,
}: {
  tone?: Exclude<Tone, 'neutral'>
  children: ReactNode
  detail?: string | null
}) {
  const { cls, icon: IconCmp } = NOTICE[tone]
  return (
    <div className={cx(s.notice, cls)} role={tone === 'error' ? 'alert' : 'status'}>
      <IconCmp size={16} className={s.noticeIcon} />
      <div>
        <div>{children}</div>
        {detail && (
          <details className={s.noticeDetail}>
            <summary>자세히</summary>
            <pre className="selectable">{detail}</pre>
          </details>
        )}
      </div>
    </div>
  )
}

/** 명령 오류를 그대로 보여주는 안내 */
export function ErrorNotice({ error }: { error: unknown }) {
  if (!error) return null
  return (
    <Notice tone="error" detail={errorDetail(error)}>
      {errorMessage(error)}
    </Notice>
  )
}

/* ---------- Empty ---------- */

export function Empty({
  icon: IconCmp,
  title,
  description,
  action,
}: {
  icon?: LucideIcon
  title: string
  description?: string
  action?: ReactNode
}) {
  return (
    <div className={s.empty}>
      {IconCmp && (
        <div className={s.emptyIcon}>
          <IconCmp size={22} />
        </div>
      )}
      <div className={s.emptyTitle}>{title}</div>
      {description && <div className={s.emptyDesc}>{description}</div>}
      {action}
    </div>
  )
}

/* ---------- Table ---------- */

export function TableWrap({ children }: { children: ReactNode }) {
  return (
    <div className={s.tableWrap}>
      <table className={s.table}>{children}</table>
    </div>
  )
}

export const tableClass = {
  num: s.num,
  center: s.center,
  muted: s.muted,
  rowActive: s.rowActive,
}

/* ---------- Tabs ---------- */

export interface TabDef<K extends string> {
  key: K
  label: string
  /** 탭 이름 옆에 보여줄 개수 (0이면 감춘다) */
  count?: number
}

export function Tabs<K extends string>({
  tabs,
  active,
  onChange,
}: {
  tabs: TabDef<K>[]
  active: K
  onChange: (key: K) => void
}) {
  return (
    <div className={s.tabs} role="tablist">
      {tabs.map((t) => (
        <button
          key={t.key}
          type="button"
          role="tab"
          aria-selected={t.key === active}
          className={t.key === active ? `${s.tab} ${s.tabActive}` : s.tab}
          onClick={() => onChange(t.key)}
        >
          {t.label}
          {t.count ? <span className={s.tabCount}>{t.count}</span> : null}
        </button>
      ))}
    </div>
  )
}

/* ---------- Drawer ---------- */

export function Drawer({
  title,
  subtitle,
  onClose,
  tabs,
  footer,
  children,
}: {
  title: string
  subtitle?: ReactNode
  onClose: () => void
  tabs?: ReactNode
  footer?: ReactNode
  children: ReactNode
}) {
  // Esc 로 닫는다 — 업무 중 손이 마우스를 떠나지 않도록
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [onClose])

  return (
    <>
      <div className={s.scrim} onClick={onClose} aria-hidden="true" />
      <aside className={s.drawer} role="dialog" aria-label={title}>
        <header className={s.drawerHead}>
          <div className={s.drawerTitle}>{title}</div>
          {subtitle}
          <button type="button" className={s.iconBtn} onClick={onClose} aria-label="닫기">
            <X size={18} />
          </button>
        </header>
        {tabs}
        <div className={s.drawerBody}>{children}</div>
        {footer && <footer className={s.drawerFoot}>{footer}</footer>}
      </aside>
    </>
  )
}

export function DrawerSpacer() {
  return <span className={s.drawerFootSpacer} />
}
