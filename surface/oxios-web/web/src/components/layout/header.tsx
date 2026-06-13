import { Link } from '@tanstack/react-router'
import { Menu, Monitor, Moon, Settings, Sun } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { useSidebarStore } from '@/stores/sidebar'
import { useThemeStore } from '@/stores/theme'
import { LanguageSelector } from './language-selector'
import { ModeTabs } from './mode-tabs'
import { NotificationBell } from './notification-bell'

export function Header() {
  const { t } = useTranslation()
  const { setMobileOpen } = useSidebarStore()

  return (
    <header className="flex h-14 items-center gap-4 border-b bg-background px-4 lg:px-6">
      {/* Mobile hamburger */}
      <button
        type="button"
        className="lg:hidden"
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

      {/* Global actions — desktop: individual icons, mobile: consolidated dropdown */}
      <div className="hidden lg:flex items-center gap-1">
        <ThemeToggle />
        <LanguageSelector />
        <NotificationBell />
        <SettingsLink />
      </div>

      {/* Mobile: consolidated settings */}
      <div className="lg:hidden">
        <MobileSettingsDropdown />
      </div>
    </header>
  )
}

// ---------------------------------------------------------------------------
// Theme toggle (icon button)
// ---------------------------------------------------------------------------

function ThemeToggle() {
  const { theme, resolved, setTheme } = useThemeStore()
  const { t } = useTranslation()

  const nextLabel =
    theme === 'dark'
      ? t('common.light', 'Light')
      : theme === 'light'
        ? t('common.system', 'System')
        : t('common.dark', 'Dark')

  return (
    <button
      type="button"
      onClick={() => {
        const next = theme === 'dark' ? 'light' : theme === 'light' ? 'system' : 'dark'
        setTheme(next)
      }}
      className="inline-flex items-center justify-center rounded-md p-2 text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-all focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
      aria-label={`${t('common.toggleTheme')}: ${nextLabel}`}
      title={nextLabel}
    >
      {theme === 'system' ? (
        <Monitor className="h-4 w-4" />
      ) : resolved === 'dark' ? (
        <Sun className="h-4 w-4" />
      ) : (
        <Moon className="h-4 w-4" />
      )}
    </button>
  )
}

// ---------------------------------------------------------------------------
// Settings link (icon button)
// ---------------------------------------------------------------------------

function SettingsLink() {
  const { t } = useTranslation()

  return (
    <Link
      to="/settings"
      search={{ section: undefined }}
      className="inline-flex items-center justify-center rounded-md p-2 text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-all focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
      aria-label={t('common.settings')}
      title={t('common.settings')}
    >
      <Settings className="h-4 w-4" />
    </Link>
  )
}

// ---------------------------------------------------------------------------
// Mobile settings dropdown (consolidated)
// ---------------------------------------------------------------------------

function MobileSettingsDropdown() {
  const { t } = useTranslation()
  const { i18n } = useTranslation()
  const { theme, resolved, setTheme } = useThemeStore()

  const themeIcon =
    theme === 'system' ? (
      <Monitor className="h-4 w-4" />
    ) : resolved === 'dark' ? (
      <Sun className="h-4 w-4" />
    ) : (
      <Moon className="h-4 w-4" />
    )

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <span className="inline-flex items-center justify-center rounded-md p-2 text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-all focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring">
          <Settings className="h-4 w-4" />
        </span>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuItem
          onClick={() => {
            const next = theme === 'dark' ? 'light' : theme === 'light' ? 'system' : 'dark'
            setTheme(next)
          }}
          className="flex items-center gap-2"
        >
          {themeIcon}
          <span>{t('common.toggleTheme')}</span>
        </DropdownMenuItem>
        <DropdownMenuItem
          onClick={() => {
            const next = i18n.language === 'ko' ? 'en' : 'ko'
            i18n.changeLanguage(next)
          }}
          className="flex items-center gap-2"
        >
          <span>🌐</span>
          <span>{i18n.language === 'ko' ? 'English' : '한국어'}</span>
        </DropdownMenuItem>
        <DropdownMenuSeparator />
        <DropdownMenuItem asChild>
          <Link to="/settings" search={{ section: undefined }}>
            <Settings className="h-4 w-4 mr-2" />
            {t('common.settings')}
          </Link>
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
