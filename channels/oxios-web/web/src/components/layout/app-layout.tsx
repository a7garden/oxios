import { Outlet } from '@tanstack/react-router'
import { cn } from '@/lib/utils'
import { useSidebarStore } from '@/stores/sidebar'
import { Header } from './header'
import { Sidebar } from './sidebar'

export function AppLayout() {
  const { mobileOpen, setMobileOpen } = useSidebarStore()

  return (
    <div className="flex h-screen overflow-hidden">
      {/* Mobile overlay */}
      {mobileOpen && (
        <div
          role="dialog"
          aria-label="Close menu"
          className="fixed inset-0 z-40 bg-black/50 lg:hidden"
          onClick={() => setMobileOpen(false)}
          onKeyDown={(e) => { if (e.key === 'Escape') setMobileOpen(false) }}
        />
      )}
      {/* Sidebar */}
      <div className={cn('hidden lg:flex', mobileOpen && 'fixed inset-y-0 left-0 z-50 flex')}>
        <Sidebar />
      </div>
      {/* Main */}
      <div className="flex flex-1 flex-col overflow-hidden">
        <Header />
        <main className="flex-1 overflow-y-auto p-4 lg:p-6">
          <Outlet />
        </main>
      </div>
    </div>
  )
}
