import { Menu, Search, Zap } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { useCommandPaletteStore } from '@/stores/command-palette'
import { useQuickAskStore } from '@/stores/quick-ask'
import { useSidebarStore } from '@/stores/sidebar'
import { MenuClock } from './menu-clock'
import { ModeTabs } from './mode-tabs'

export function Header() {
  const { t } = useTranslation()
  const { setMobileOpen } = useSidebarStore()
  const openPalette = useCommandPaletteStore((s) => s.openPalette)
  const openQuickAsk = useQuickAskStore((s) => s.openQuickAsk)

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

      {/* Global command palette trigger (⌘K) — discoverability for the power-user feature */}
      {/* QuickAsk (⌘J) — one-shot throwaway question, no session persisted */}
      <Button
        type="button"
        variant="outline"
        size="sm"
        onClick={() => openQuickAsk()}
        className="h-8 gap-1 px-2.5 text-muted-foreground"
        aria-label={t('quickAsk.openAria')}
        title={`${t('quickAsk.openAria')} (⌘J)`}
      >
        <Zap className="h-3.5 w-3.5" />
        <kbd className="hidden sm:inline-flex h-5 items-center rounded border border-border bg-muted/50 px-1.5 font-mono text-[10px]">
          ⌘J
        </kbd>
      </Button>
      <Button
        type="button"
        variant="outline"
        size="sm"
        onClick={() => openPalette()}
        className="h-8 gap-2 px-2.5 text-muted-foreground"
        aria-label={t('commandPalette.openAria')}
      >
        <Search className="h-3.5 w-3.5" />
        <span className="hidden sm:inline text-xs">{t('commandPalette.placeholder')}</span>
        <kbd className="hidden sm:inline-flex h-5 items-center rounded border border-border bg-muted/50 px-1.5 font-mono text-[10px]">
          ⌘K
        </kbd>
      </Button>

      {/* Menu-bar clock — single trigger for Notification Center */}
      <MenuClock />
    </header>
  )
}
