import { Outlet, useRouterState } from '@tanstack/react-router'
import React from 'react'
import { useTranslation } from 'react-i18next'
import { InfoPanel } from '@/components/knowledge/info-panel'
import { MoveModal } from '@/components/knowledge/move-modal'
import { SearchModal } from '@/components/knowledge/search-modal'
import { useApprovalWatcher, useGlobalEvents } from '@/hooks/use-global-events'
import { useKnowledgeShortcuts } from '@/hooks/use-knowledge-shortcuts'
import { cn } from '@/lib/utils'
import { useEventStore } from '@/stores/events'
import { useKnowledgeStore } from '@/stores/knowledge'
import { useSidebarStore } from '@/stores/sidebar'
import { Header } from './header'
import { Sidebar } from './sidebar'

/**
 * AppLayout — single sidebar, mode-adaptive.
 *
 * The Sidebar component internally switches between Console/Knowledge/Chat
 * nav content based on the current route. No sidebar replacement needed.
 */
export function AppLayout() {
  const { t } = useTranslation()
  const { mobileOpen, setMobileOpen } = useSidebarStore()

  const router = useRouterState()
  const pathname = router.location.pathname
  const isKnowledge = pathname.startsWith('/knowledge')
  const isKnowledgeSubRoute = isKnowledge && pathname !== '/knowledge' && pathname !== '/knowledge/'
  const isChat = pathname === '/chat'

  const { infoPanelOpen } = useKnowledgeStore()

  useKnowledgeShortcuts()
  useGlobalEvents()
  useApprovalWatcher()

  // Bootstrap singleton SSE connection on first mount
  const connectEvents = useEventStore((s) => s.connect)
  React.useState(() => {
    connectEvents()
  })

  return (
    <div className="flex h-screen overflow-hidden">
      {/* ── Sidebar — single, mode-adaptive ── */}
      {mobileOpen && (
        <div
          role="dialog"
          aria-label={t('common.closeMenu')}
          className="fixed inset-0 z-40 bg-black/50 backdrop-blur-sm lg:hidden animate-in fade-in-0 duration-200"
          onClick={() => setMobileOpen(false)}
          onKeyDown={(e) => {
            if (e.key === 'Escape') setMobileOpen(false)
          }}
        />
      )}
      <div
        className={cn(
          'hidden lg:flex',
          mobileOpen &&
            'fixed inset-y-0 left-0 z-50 flex flex-col bg-sidebar animate-in slide-in-from-left duration-300',
        )}
      >
        <Sidebar />
      </div>

      {/* ── Main content area ── */}
      <div className="flex flex-1 flex-col min-w-0 overflow-hidden">
        <Header />

        {isKnowledge ? (
          <div className="flex flex-1 min-h-0 overflow-hidden">
            <div className="flex flex-1 min-w-0 overflow-hidden">
              <Outlet />
            </div>
            {/* InfoPanel only on main knowledge route, not sub-routes */}
            {!isKnowledgeSubRoute && infoPanelOpen && <InfoPanel />}
          </div>
        ) : isChat ? (
          /* Chat: no padding, full height */
          <main className="flex-1 min-h-0 overflow-hidden">
            <Outlet />
          </main>
        ) : (
          <main className="flex-1 overflow-y-auto p-3 lg:p-4 min-h-0">
            <Outlet />
          </main>
        )}
      </div>

      {isKnowledge && (
        <>
          <SearchModal />
          <MoveModal />
        </>
      )}
    </div>
  )
}
