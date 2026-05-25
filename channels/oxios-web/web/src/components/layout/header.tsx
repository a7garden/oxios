import { Link, useRouterState } from '@tanstack/react-router'
import { ArrowLeft, Menu } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Separator } from '@/components/ui/separator'
import { cn } from '@/lib/utils'
import { useKnowledgeStore } from '@/stores/knowledge'
import { useSidebarStore } from '@/stores/sidebar'
import { NotificationBell } from './notification-bell'
import { LanguageSelector } from './language-selector'

export function Header() {
  const { t } = useTranslation()
  const { setMobileOpen } = useSidebarStore()
  const toggleKnowledgeSidebar = useKnowledgeStore((s) => s.toggleSidebar)
  const router = useRouterState()
  const pathname = router.location.pathname
  const isKnowledge = pathname.startsWith('/knowledge')

  /** Mobile hamburger: opens the correct sidebar for the current mode */
  const handleMobileMenu = () => {
    if (isKnowledge) {
      toggleKnowledgeSidebar()
    } else {
      setMobileOpen(true)
    }
  }

  return (
    <header className="flex h-14 items-center gap-4 border-b bg-background px-4 lg:px-6">
      <button
        type="button"
        className="lg:hidden"
        onClick={handleMobileMenu}
        aria-label={isKnowledge ? t('common.toggleSidebar', 'Toggle sidebar') : t('common.openNav', 'Open navigation menu')}
      >
        <Menu className="h-5 w-5" />
      </button>

      {isKnowledge ? (
        <KnowledgeBreadcrumb />
      ) : (
        <Separator orientation="vertical" className="hidden lg:block h-6" />
      )}

      <div className="flex-1" />

      {/* Global notification bell */}
      <NotificationBell />

      {/* Language selector */}
      <LanguageSelector />

      <div className="flex items-center gap-2 text-sm text-muted-foreground">
        <div className="h-2 w-2 rounded-full bg-emerald-500" aria-hidden="true" />
        <span>{t('common.oxiosBrand')}</span>
      </div>
    </header>
  )
}

/**
 * Knowledge-specific breadcrumb. Extracted into its own component so that
 * useKnowledgeStore subscriptions only activate on knowledge routes,
 * avoiding unnecessary re-renders on dashboard pages.
 */
function KnowledgeBreadcrumb() {
  const { t } = useTranslation()
  const currentFilePath = useKnowledgeStore((s) => s.currentFilePath)
  const mode = useKnowledgeStore((s) => s.mode)

  return (
    <>
      <Link
        to="/"
        className="flex items-center gap-1.5 text-sm text-muted-foreground hover:text-foreground transition-colors"
      >
        <ArrowLeft className="h-4 w-4" />
        <span>{t('common.dashboard', 'Dashboard')}</span>
      </Link>
      <Separator orientation="vertical" className="h-6" />
      <div className="flex items-center gap-2 text-sm">
        <span className={cn('font-medium', !currentFilePath && 'text-foreground')}>{t('knowledge.title', 'Knowledge')}</span>
        {currentFilePath && (
          <>
            <span className="text-muted-foreground">/</span>
            <span className="text-muted-foreground truncate max-w-[200px]">
              {currentFilePath.replace(/\.md$/, '')}
            </span>
          </>
        )}
        {mode === 'chat' && !currentFilePath && (
          <>
            <span className="text-muted-foreground">/</span>
            <span className="text-muted-foreground">{t('chat.title', 'Chat')}</span>
          </>
        )}
      </div>
    </>
  )
}