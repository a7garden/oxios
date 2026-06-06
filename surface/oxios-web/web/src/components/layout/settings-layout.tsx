import { ChevronDown } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { cn } from '@/lib/utils'

// ─── Types ────────────────────────────────────────────────────

export interface SubNavItem {
  id: string
  labelKey: string
  icon?: React.ReactNode
}

export interface SubNavGroup {
  /** Unique id — first item in the group triggers the group label */
  id: string
  labelKey: string
  items: SubNavItem[]
}

// ─── Mobile dropdown selector ─────────────────────────────────

function MobileSelector({
  groups,
  activeId,
  onSelect,
}: {
  groups: SubNavGroup[]
  activeId: string
  onSelect: (id: string) => void
}) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const ref = useRef<HTMLDivElement>(null)

  useEffect(() => {
    function handleClick(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false)
    }
    if (open) document.addEventListener('mousedown', handleClick)
    return () => document.removeEventListener('mousedown', handleClick)
  }, [open])

  // Find current label
  const current = groups.flatMap((g) => g.items).find((i) => i.id === activeId)
  const currentLabel = current ? t(current.labelKey) : ''

  return (
    <div ref={ref} className="relative lg:hidden">
      <button
        type="button"
        onClick={() => setOpen(!open)}
        className="flex w-full items-center justify-between rounded-lg border bg-muted/50 px-4 py-2.5 text-sm font-medium"
      >
        <span className="flex items-center gap-2">
          {current?.icon}
          {currentLabel}
        </span>
        <ChevronDown
          className={cn('h-4 w-4 text-muted-foreground transition-transform', open && 'rotate-180')}
        />
      </button>

      {open && (
        <div className="absolute z-50 mt-1 w-full rounded-lg border bg-popover shadow-lg overflow-hidden">
          {groups.map((group) => (
            <div key={group.id}>
              <div className="px-4 py-1.5 text-2xs font-semibold uppercase tracking-wider text-muted-foreground/60 bg-muted/30">
                {t(group.labelKey)}
              </div>
              {group.items.map((item) => (
                <button
                  key={item.id}
                  type="button"
                  className={cn(
                    'flex w-full items-center gap-2.5 px-4 py-2.5 text-sm transition-colors',
                    activeId === item.id
                      ? 'bg-primary/10 text-primary font-medium'
                      : 'text-foreground hover:bg-muted/50',
                  )}
                  onClick={() => {
                    onSelect(item.id)
                    setOpen(false)
                  }}
                >
                  {item.icon}
                  {t(item.labelKey)}
                </button>
              ))}
            </div>
          ))}
        </div>
      )}
    </div>
  )
}

// ─── Desktop sidebar nav ──────────────────────────────────────

function DesktopNav({
  groups,
  activeId,
  onSelect,
}: {
  groups: SubNavGroup[]
  activeId: string
  onSelect: (id: string) => void
}) {
  const { t } = useTranslation()

  return (
    <nav className="hidden lg:block w-52 shrink-0">
      <div className="sticky top-0 space-y-1">
        {groups.map((group) => (
          <div key={group.id}>
            <div className="px-3 pt-4 pb-1 first:pt-0">
              <span className="text-2xs font-semibold uppercase tracking-wider text-muted-foreground/60">
                {t(group.labelKey)}
              </span>
            </div>
            {group.items.map((item) => (
              <button
                key={item.id}
                type="button"
                onClick={() => onSelect(item.id)}
                className={cn(
                  'flex w-full items-center gap-2.5 rounded-lg px-3 py-2 text-sm transition-colors',
                  activeId === item.id
                    ? 'bg-primary/10 text-primary font-medium'
                    : 'text-muted-foreground hover:bg-muted/50 hover:text-foreground',
                )}
              >
                <span className="shrink-0">{item.icon}</span>
                <span className="truncate">{t(item.labelKey)}</span>
              </button>
            ))}
          </div>
        ))}
      </div>
    </nav>
  )
}

// ─── Layout (reusable) ────────────────────────────────────────

/**
 * Two-pane settings-style layout.
 *
 * Desktop (≥1024px): Fixed sidebar nav on the left, scrollable content on the right.
 * Mobile: Dropdown selector at the top, content below.
 *
 * Usage:
 * ```tsx
 * <SettingsLayout
 *   groups={[
 *     { id: 'general', labelKey: 'settings.groupGeneral', items: [
 *       { id: 'engine', labelKey: 'settings.engine', icon: <Bot /> },
 *       { id: 'kernel', labelKey: 'settings.kernel', icon: <Cpu /> },
 *     ]},
 *   ]}
 *   activeId={activeSection}
 *   onNavigate={setActiveSection}
 * >
 *   {renderContent()}
 * </SettingsLayout>
 * ```
 */
export function SettingsLayout({
  groups,
  activeId,
  onNavigate,
  children,
}: {
  groups: SubNavGroup[]
  activeId: string
  onNavigate: (id: string) => void
  children: React.ReactNode
}) {
  return (
    <div className="space-y-4">
      {/* Mobile: dropdown selector */}
      <MobileSelector groups={groups} activeId={activeId} onSelect={onNavigate} />

      <div className="flex gap-6">
        {/* Desktop: sidebar nav */}
        <DesktopNav groups={groups} activeId={activeId} onSelect={onNavigate} />

        {/* Content */}
        <div className="flex-1 min-w-0 max-w-3xl">{children}</div>
      </div>
    </div>
  )
}
