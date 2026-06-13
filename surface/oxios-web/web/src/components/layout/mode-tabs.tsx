import { Link, useRouterState } from '@tanstack/react-router'
import { LayoutDashboard, MessageSquare, NotebookPen } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { cn } from '@/lib/utils'
import type { SidebarMode } from '@/stores/sidebar'

const MODES: {
  key: SidebarMode
  icon: typeof LayoutDashboard
  labelKey: string
  href: string
}[] = [
  { key: 'console', icon: LayoutDashboard, labelKey: 'sidebar.console', href: '/' },
  { key: 'knowledge', icon: NotebookPen, labelKey: 'sidebar.knowledge', href: '/knowledge' },
  { key: 'chat', icon: MessageSquare, labelKey: 'sidebar.chat', href: '/chat' },
]

interface ModeTabsProps {
  variant?: 'header' | 'sidebar'
}

export function ModeTabs({ variant = 'header' }: ModeTabsProps) {
  const { t } = useTranslation()
  const router = useRouterState()
  const pathname = router.location.pathname

  const currentMode: SidebarMode = pathname.startsWith('/knowledge')
    ? 'knowledge'
    : pathname === '/chat'
      ? 'chat'
      : 'console'

  return (
    <nav
      aria-label={t('common.modeNavigation', 'Mode navigation')}
      className={cn(
        'flex items-center',
        variant === 'header' ? 'gap-0.5 rounded-lg bg-muted/60 p-0.5' : 'gap-0.5',
      )}
    >
      {MODES.map(({ key, icon: Icon, labelKey, href }) => {
        const isActive = currentMode === key
        return (
          <Link
            key={key}
            to={href}
            aria-current={isActive ? 'page' : undefined}
            className={cn(
              'flex items-center gap-1.5 px-3 py-1.5 text-sm font-medium transition-all',
              variant === 'header' && [
                'rounded-md',
                isActive
                  ? 'bg-background shadow-sm text-foreground'
                  : 'text-muted-foreground hover:text-foreground',
              ],
              variant === 'sidebar' && [
                'flex-1 justify-center rounded-md',
                isActive
                  ? 'bg-sidebar-accent text-sidebar-accent-foreground'
                  : 'text-sidebar-foreground/50 hover:bg-sidebar-accent/50',
              ],
            )}
          >
            <Icon className="h-3.5 w-3.5" />
            <span>{t(labelKey)}</span>
          </Link>
        )
      })}
    </nav>
  )
}
