import { Outlet, useRouterState } from '@tanstack/react-router'
import { Menu } from 'lucide-react'
import { InfoPanel } from '@/components/knowledge/info-panel'
import { KnowledgeSidebar } from '@/components/knowledge/knowledge-sidebar'
import { MoveModal } from '@/components/knowledge/move-modal'
import { SearchModal } from '@/components/knowledge/search-modal'
import { useKnowledgeShortcuts } from '@/hooks/use-knowledge-shortcuts'
import { useGlobalEvents, useApprovalWatcher } from '@/hooks/use-global-events'
import { cn } from '@/lib/utils'
import { useKnowledgeStore } from '@/stores/knowledge'
import { useSidebarStore } from '@/stores/sidebar'
import { Header } from './header'
import { Sidebar } from './sidebar'

/**
 * Unified AppLayout that seamlessly switches between Dashboard and Knowledge modes.
 *
 * Dashboard mode: Standard sidebar + header + outlet
 * Knowledge mode: Knowledge sidebar replaces main sidebar, knowledge content fills the outlet area
 */
export function AppLayout() {
  const { mobileOpen, setMobileOpen } = useSidebarStore()

  // Single router subscription (B3 fix — consolidate two calls into one)
  const router = useRouterState()
  const pathname = router.location.pathname
  const isKnowledge = pathname.startsWith('/knowledge')
  const isKnowledgeSubRoute = isKnowledge && pathname !== '/knowledge' && pathname !== '/knowledge/'

  const { sidebarOpen, toggleSidebar, infoPanelOpen } = useKnowledgeStore()

  // Always call the hook — it guards internally via pathnameRef
  useKnowledgeShortcuts()

  // Global event → notification pipeline
  useGlobalEvents()
  useApprovalWatcher()

  return (
    <div className="flex h-screen overflow-hidden">
      {/* ── Sidebar area ── */}
      {isKnowledge ? (
        <>
          {/* Mobile overlay */}
          {sidebarOpen && (
            <div
              role="dialog"
              aria-label="Close sidebar"
              className="fixed inset-0 z-40 bg-black/50 lg:hidden"
              onClick={() => toggleSidebar()}
              onKeyDown={(e) => {
                if (e.key === 'Escape') toggleSidebar()
              }}
            />
          )}
          {sidebarOpen ? (
            <div
              className={cn(
                'flex shrink-0',
                // Mobile: fixed full-width overlay
                'fixed inset-y-0 left-0 z-50 w-80 max-w-[85vw] lg:relative lg:z-auto lg:max-w-none',
              )}
            >
              <KnowledgeSidebar />
            </div>
          ) : (
            <button
              type="button"
              onClick={toggleSidebar}
              className={cn(
                'shrink-0 items-center justify-center border-r bg-background hover:bg-accent/50 transition-colors cursor-pointer',
                // B1 fix: show on all screen sizes. Desktop: 18px strip. Mobile: 36px tap target.
                'flex w-[36px] lg:w-[18px]',
              )}
              aria-label="Open sidebar"
            >
              <Menu className="h-5 w-5 text-muted-foreground lg:hidden" />
              <span className="hidden lg:block text-muted-foreground text-xs rotate-90 whitespace-nowrap">
                Notes
              </span>
            </button>
          )}
        </>
      ) : (
        <>
          {mobileOpen && (
            <div
              role="dialog"
              aria-label="Close menu"
              className="fixed inset-0 z-40 bg-black/50 lg:hidden"
              onClick={() => setMobileOpen(false)}
              onKeyDown={(e) => {
                if (e.key === 'Escape') setMobileOpen(false)
              }}
            />
          )}
          <div className={cn('hidden lg:flex', mobileOpen && 'fixed inset-y-0 left-0 z-50 flex')}>
            <Sidebar />
          </div>
        </>
      )}

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
        ) : (
          <main className="flex-1 overflow-y-auto p-4 lg:p-6">
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
