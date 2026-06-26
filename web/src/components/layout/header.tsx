import { Menu } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { useSidebarStore } from '@/stores/sidebar'
import { MenuClock } from './menu-clock'
import { ModeTabs } from './mode-tabs'

export function Header() {
  const { t } = useTranslation()
  const { setMobileOpen } = useSidebarStore()

  return (
    <header className="flex h-14 items-center gap-4 border-b bg-background px-4 lg:px-6 pt-[env(safe-area-inset-top)]">
      {/* Mobile hamburger */}
      <button
        type="button"
        className="lg:hidden rounded-md focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
        onClick={() => setMobileOpen(true)}
        aria-label={t('common.openNav', 'Open navigation menu')}
      >
        <Menu className="h-5 w-5" />
      </button>

      {/* Mode tabs — desktop only */}
      <div className="hidden lg:block">
        <ModeTabs variant="header" />
      </div>

      <div className="flex-1" />

      {/* Menu-bar clock — single trigger for Notification Center */}
      <MenuClock />
    </header>
  )
}
