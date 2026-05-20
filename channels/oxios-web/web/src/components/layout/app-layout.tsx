import { Outlet, useRouterState } from '@tanstack/react-router'
import { cn } from '@/lib/utils'
import { useSidebarStore } from '@/stores/sidebar'
import { useKnowledgeStore } from '@/stores/knowledge'
import { Header } from './header'
import { Sidebar } from './sidebar'
import { KnowledgeSidebar } from '@/components/knowledge/knowledge-sidebar'
import { InfoPanel } from '@/components/knowledge/info-panel'
import { SearchModal } from '@/components/knowledge/search-modal'
import { MoveModal } from '@/components/knowledge/move-modal'
import { useKnowledgeShortcuts } from '@/hooks/use-knowledge-shortcuts'

/**
 * Detect if current route is within the Knowledge section.
 */
function useIsKnowledgeRoute() {
  const router = useRouterState()
  const pathname = router.location.pathname
  return pathname.startsWith('/knowledge')
}

/**
 * Unified AppLayout that seamlessly switches between Dashboard and Knowledge modes.
 *
 * Dashboard mode: Standard sidebar + header + outlet
 * Knowledge mode: Knowledge sidebar replaces main sidebar, knowledge content fills the outlet area
 */
export function AppLayout() {
  const { mobileOpen, setMobileOpen } = useSidebarStore()
  const isKnowledge = useIsKnowledgeRoute()
  const { sidebarOpen, toggleSidebar, infoPanelOpen } = useKnowledgeStore()

  // Always call the hook — it's a no-op when not in knowledge section
  useKnowledgeShortcuts()

  return (
    <div className="flex h-screen overflow-hidden">
      {/* ── Sidebar area ── */}
      {isKnowledge ? (
        sidebarOpen ? (
          <KnowledgeSidebar />
        ) : (
          <button
            type="button"
            onClick={toggleSidebar}
            className="hidden lg:flex w-[18px] shrink-0 items-center justify-center border-r bg-background hover:bg-accent/50 transition-colors cursor-pointer"
            aria-label="Open sidebar"
          >
            <span className="text-muted-foreground text-xs rotate-90 whitespace-nowrap">Notes</span>
          </button>
        )
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
            {infoPanelOpen && <InfoPanel />}
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
