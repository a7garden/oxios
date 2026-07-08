import { Link, useRouterState } from '@tanstack/react-router'
import {
  Activity,
  Bell,
  BookOpen,
  Bot,
  Brain,
  FilePlus,
  Flame,
  FolderKanban,
  FolderOpen,
  FolderPlus,
  GitBranch,
  LayoutDashboard,
  LayoutGrid,
  Mail,
  MessageSquare,
  Network,
  PanelLeft,
  PanelLeftClose,
  Settings,
  Theater,
  Timer,
  Trash2,
  Wallet,
  Zap,
} from 'lucide-react'
import React, { useCallback, useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { FileTree } from '@/components/knowledge/file-tree'
import { HabitsDialog } from '@/components/knowledge/habits-dialog'
import { KnowledgeSettingsDialog } from '@/components/knowledge/knowledge-settings-dialog'
import { Button } from '@/components/ui/button'
import { Separator } from '@/components/ui/separator'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import {
  useDeleteFile,
  useJournalToday,
  useKnowledgeTree,
  useWriteFile,
} from '@/hooks/use-knowledge'
import { cn } from '@/lib/utils'
import { useKnowledgeStore } from '@/stores/knowledge'
import { deriveSidebarMode, useSidebarStore } from '@/stores/sidebar'
import { ChatSessionNav } from './chat-session-nav'
import { ModeTabs } from './mode-tabs'
import { SidebarFooter } from './sidebar-footer'

// ── Types ──────────────────────────────────────────────────────

export interface NavItem {
  labelKey: string
  href: string
  icon: React.ReactNode
  show?: boolean
  badge?: number
}

// ── Sidebar design primitives ─────────────────────────────────
//
// Shared tokens for all three sidebar modes (Console, Knowledge, Chat).
// Every item, section header, and separator must use these constants
// so the three modes feel visually identical.
//

/** Primary navigation item (icon + label). */
export const itemBase =
  'flex items-center gap-3 rounded-lg px-2.5 py-2 text-sm w-full text-left select-none transition-all focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring focus-visible:ring-offset-1 focus-visible:ring-offset-sidebar'

/** Dense list item (session rows, file rows). */
export const itemDense =
  'flex items-center gap-2 rounded-lg px-2.5 py-1.5 text-xs w-full text-left select-none transition-all focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring'

export const itemActive = 'bg-sidebar-accent text-sidebar-accent-foreground font-medium'
export const itemInactive =
  'text-sidebar-foreground/70 hover:bg-sidebar-accent/50 hover:text-sidebar-foreground'
export const itemCollapsedBase =
  'flex items-center justify-center rounded-lg p-2 select-none transition-all focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring'

/** Section header label. */
export const sectionHeader =
  'px-2 mb-1 text-xs font-medium text-muted-foreground uppercase tracking-wider select-none'

/** Vertical spacing between sections. */
export const sectionGap = 'mb-3'

/** Horizontal separator between sections. */
export const sectionSeparator = 'border-t border-sidebar-border my-2'

// ── Console mode nav groups ────────────────────────────────────

export const consoleNavGroups: { labelKey: string; items: NavItem[] }[] = [
  {
    labelKey: 'common.main',
    items: [
      { labelKey: 'common.dashboard', href: '/', icon: <LayoutDashboard className="h-4 w-4" /> },
    ],
  },
  {
    labelKey: 'common.agents',
    items: [
      { labelKey: 'common.agents', href: '/agents', icon: <Bot className="h-4 w-4" /> },
      { labelKey: 'common.personas', href: '/personas', icon: <Theater className="h-4 w-4" /> },
      { labelKey: 'common.skills', href: '/skills', icon: <Zap className="h-4 w-4" /> },
    ],
  },
  {
    labelKey: 'common.projects',
    items: [
      {
        labelKey: 'common.projects',
        href: '/projects',
        icon: <FolderKanban className="h-4 w-4" />,
      },
      {
        labelKey: 'common.mounts',
        href: '/mounts',
        icon: <FolderPlus className="h-4 w-4" />,
      },
    ],
  },
  {
    labelKey: 'common.storage',
    items: [
      { labelKey: 'common.memory', href: '/memory', icon: <Brain className="h-4 w-4" /> },
      {
        labelKey: 'common.workspace',
        href: '/workspace',
        icon: <FolderOpen className="h-4 w-4" />,
      },
    ],
  },
  {
    labelKey: 'common.operations',
    items: [
      { labelKey: 'common.cronJobs', href: '/cron-jobs', icon: <Timer className="h-4 w-4" /> },
      { labelKey: 'common.cost', href: '/budget', icon: <Wallet className="h-4 w-4" /> },
      {
        labelKey: 'common.tokenMaxing',
        href: '/token-maxing',
        icon: <Flame className="h-4 w-4" />,
      },
    ],
  },
  {
    labelKey: 'common.infrastructure',
    items: [
      { labelKey: 'common.mcpServers', href: '/mcp', icon: <Zap className="h-4 w-4" /> },
      { labelKey: 'common.email', href: '/email', icon: <Mail className="h-4 w-4" /> },
      { labelKey: 'common.git', href: '/git', icon: <GitBranch className="h-4 w-4" /> },
    ],
  },
  {
    labelKey: 'common.system',
    items: [
      { labelKey: 'common.resources', href: '/resources', icon: <Activity className="h-4 w-4" /> },
      { labelKey: 'common.security', href: '/security', icon: <Bell className="h-4 w-4" /> },
      { labelKey: 'common.settings', href: '/settings', icon: <Settings className="h-4 w-4" /> },
    ],
  },
]

// ── Sidebar component ──────────────────────────────────────────

export function Sidebar() {
  const { collapsed, toggle, mode, setMode, mobileOpen } = useSidebarStore()
  const router = useRouterState()
  const currentPath = router.location.pathname

  // Sync mode from route
  useEffect(() => {
    const derivedMode = deriveSidebarMode(currentPath)
    setMode(derivedMode)
  }, [currentPath, setMode])

  return (
    <aside
      className={cn(
        'flex h-full w-72 max-w-[85vw] flex-col overflow-hidden border-r bg-sidebar text-sidebar-foreground transition-[width] duration-300 ease-[var(--animate-in-easing)]',
        // Desktop collapses to icon rail; mobile drawer stays full width
        collapsed ? 'lg:w-16 lg:max-w-none' : 'lg:w-60 lg:max-w-none',
      )}
    >
      {/* Header — brand + collapse toggle */}
      <div
        className={cn(
          'flex h-14 items-center px-3',
          collapsed && !mobileOpen ? 'justify-center' : 'justify-between',
        )}
      >
        {!(collapsed && !mobileOpen) && (
          <div className="flex items-center gap-2">
            <Zap className="h-5 w-5 text-primary" />
            <span className="font-bold text-lg">Oxios</span>
          </div>
        )}
        {/* Desktop collapse toggle */}
        <button
          type="button"
          onClick={toggle}
          className="hidden lg:block rounded-md p-1.5 hover:bg-sidebar-accent focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
          aria-label={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
        >
          {collapsed ? <PanelLeft className="h-4 w-4" /> : <PanelLeftClose className="h-4 w-4" />}
        </button>
      </div>

      {/* Desktop mode switcher — single source of truth for Console /
          Knowledge / Chat on desktop. Collapses to an icon rail (VS Code
          Activity Bar) when the sidebar is collapsed. Mobile mode switching
          lives in the BottomNav bar. */}
      <div className="hidden lg:block px-2 pb-2">
        <ModeTabs collapsed={collapsed} />
      </div>

      <Separator />

      {/* Nav content — mode-specific */}
      <nav className="flex-1 overflow-y-auto p-2">
        {mode === 'console' && <ConsoleNav />}
        {mode === 'knowledge' && <KnowledgeNav />}
        {mode === 'chat' && <ChatSessionNav />}
      </nav>

      {/* Footer — global preferences (theme / language / settings) */}
      <Separator />
      <SidebarFooter collapsed={collapsed && !mobileOpen} />
    </aside>
  )
}

// ── Console Nav ────────────────────────────────────────────────

function ConsoleNav() {
  const { t } = useTranslation()
  const router = useRouterState()
  const currentPath = router.location.pathname
  const { collapsed } = useSidebarStore()

  return (
    <>
      {consoleNavGroups.map((group) => (
        <div key={group.labelKey} className={sectionGap}>
          {!collapsed && <p className={sectionHeader}>{t(group.labelKey)}</p>}
          {group.items.map((item) => (
            <NavItemLink
              key={item.href}
              item={item}
              currentPath={currentPath}
              collapsed={collapsed}
            />
          ))}
        </div>
      ))}
    </>
  )
}

// ── Knowledge Nav ──────────────────────────────────────────────

function KnowledgeNav() {
  const { t } = useTranslation()
  const { collapsed } = useSidebarStore()
  const router = useRouterState()
  const currentPath = router.location.pathname
  const { mode, currentFilePath, openFile, openChat, openHome } = useKnowledgeStore()
  const { data: entries, isLoading, refetch } = useKnowledgeTree()
  const writeFile = useWriteFile()
  const deleteFile = useDeleteFile()
  const journalToday = useJournalToday()
  const [habitsOpen, setHabitsOpen] = useState(false)
  const [settingsOpen, setSettingsOpen] = useState(false)

  const handleNewFile = useCallback(async () => {
    const name = 'New file.md'
    await writeFile.mutateAsync({ path: name, content: `# New file\n\n` })
    openFile(name)
    refetch()
  }, [writeFile, openFile, refetch])

  const handleNewFolder = useCallback(async () => {
    const name = prompt('Enter folder name:', 'New Folder')
    if (!name?.trim()) return
    await writeFile.mutateAsync({ path: `${name.trim()}/.keep`, content: '' })
    refetch()
  }, [writeFile, refetch])

  const handleDelete = useCallback(async () => {
    if (!currentFilePath) return
    if (confirm(`Delete ${currentFilePath}?`)) {
      await deleteFile.mutateAsync(currentFilePath)
    }
  }, [deleteFile, currentFilePath])

  const handleOpenJournal = useCallback(() => {
    if (journalToday.data?.path) {
      openFile(journalToday.data.path)
    }
  }, [journalToday.data, openFile])

  // Collapsed: show minimal icons
  if (collapsed) {
    return (
      <>
        <div className="flex flex-col items-center gap-1 py-1">
          <Tooltip>
            <TooltipTrigger asChild>
              <button
                type="button"
                onClick={() => openHome()}
                className={cn(itemCollapsedBase, mode === 'home' && itemActive)}
              >
                <LayoutGrid className="h-4 w-4" />
              </button>
            </TooltipTrigger>
            <TooltipContent side="right">{t('knowledge.homeTitle')}</TooltipContent>
          </Tooltip>
          <Tooltip>
            <TooltipTrigger asChild>
              <button
                type="button"
                onClick={() => openChat()}
                className={cn(itemCollapsedBase, mode === 'chat' && itemActive)}
              >
                <MessageSquare className="h-4 w-4" />
              </button>
            </TooltipTrigger>
            <TooltipContent side="right">{t('knowledge.chatTitle', 'Quick Notes')}</TooltipContent>
          </Tooltip>
          <Tooltip>
            <TooltipTrigger asChild>
              <button
                type="button"
                onClick={handleOpenJournal}
                className={cn(itemCollapsedBase, itemInactive)}
              >
                <BookOpen className="h-4 w-4" />
              </button>
            </TooltipTrigger>
            <TooltipContent side="right">{t('knowledge.toJournal')}</TooltipContent>
          </Tooltip>
          <Tooltip>
            <TooltipTrigger asChild>
              <Link
                to="/knowledge/graph"
                className={cn(
                  itemCollapsedBase,
                  currentPath === '/knowledge/graph' ? itemActive : itemInactive,
                )}
              >
                <Network className="h-4 w-4" />
              </Link>
            </TooltipTrigger>
            <TooltipContent side="right">{t('knowledge.linkGraphTitle')}</TooltipContent>
          </Tooltip>
          <Tooltip>
            <TooltipTrigger asChild>
              <button
                type="button"
                onClick={handleNewFile}
                className={cn(itemCollapsedBase, itemInactive)}
              >
                <FilePlus className="h-4 w-4" />
              </button>
            </TooltipTrigger>
            <TooltipContent side="right">{t('knowledge.newFile')}</TooltipContent>
          </Tooltip>
          <Tooltip>
            <TooltipTrigger asChild>
              <button
                type="button"
                onClick={() => setHabitsOpen(true)}
                className={cn(itemCollapsedBase, itemInactive)}
              >
                <Activity className="h-4 w-4" />
              </button>
            </TooltipTrigger>
            <TooltipContent side="right">{t('knowledge.habitsTitle')}</TooltipContent>
          </Tooltip>
          <Tooltip>
            <TooltipTrigger asChild>
              <button
                type="button"
                onClick={() => setSettingsOpen(true)}
                className={cn(itemCollapsedBase, itemInactive)}
              >
                <Settings className="h-4 w-4" />
              </button>
            </TooltipTrigger>
            <TooltipContent side="right">{t('knowledge.knowledgeSettings')}</TooltipContent>
          </Tooltip>
        </div>
        <HabitsDialog open={habitsOpen} onOpenChange={setHabitsOpen} />
        <KnowledgeSettingsDialog open={settingsOpen} onOpenChange={setSettingsOpen} />
      </>
    )
  }

  return (
    <>
      {/* Views — Home, Quick Notes, Journal, Graph */}
      <div className={sectionGap}>
        <button
          type="button"
          onClick={() => openHome()}
          className={cn(itemBase, mode === 'home' ? itemActive : itemInactive)}
        >
          <LayoutGrid className="h-4 w-4" />
          <span>{t('knowledge.homeTitle')}</span>
        </button>
        <button
          type="button"
          onClick={() => openChat()}
          className={cn(itemBase, mode === 'chat' ? itemActive : itemInactive)}
        >
          <MessageSquare className="h-4 w-4" />
          <span>{t('knowledge.chatTitle', 'Quick Notes')}</span>
        </button>
        <button
          type="button"
          onClick={handleOpenJournal}
          disabled={journalToday.isLoading}
          className={cn(itemBase, itemInactive, 'disabled:opacity-50')}
        >
          <BookOpen className="h-4 w-4" />
          <span>{t('knowledge.toJournal')}</span>
        </button>
        <NavItemLink
          item={{
            href: '/knowledge/graph',
            icon: <Network className="h-4 w-4" />,
            labelKey: 'knowledge.linkGraphTitle',
          }}
          currentPath={currentPath}
          collapsed={collapsed}
        />
      </div>

      <div className={sectionSeparator} />

      {/* File tree */}
      <p className={sectionHeader}>{t('knowledge.files', 'Files')}</p>
      <div className="flex-1 overflow-y-auto">
        {isLoading ? (
          <div className="px-4 py-2 text-xs text-sidebar-foreground/50">
            {t('knowledge.loading')}
          </div>
        ) : entries ? (
          <FileTree entries={entries} onFileSelect={openFile} currentPath={currentFilePath} />
        ) : null}
      </div>

      {/* Action bar — file ops (left) · tools (right) */}
      <div className="flex items-center gap-0.5 pt-2 border-t border-sidebar-border mt-2 px-1">
        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7 hover:bg-sidebar-accent/50"
          onClick={handleNewFile}
          title={t('knowledge.newFileShortcut')}
        >
          <FilePlus className="h-4 w-4" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7 hover:bg-sidebar-accent/50"
          onClick={handleNewFolder}
          title={t('knowledge.newFolderShortcut')}
        >
          <FolderPlus className="h-4 w-4" />
        </Button>
        {currentFilePath && (
          <Button
            variant="ghost"
            size="icon"
            className="h-7 w-7 text-destructive hover:bg-sidebar-accent/50"
            onClick={handleDelete}
            title={t('knowledge.deleteCurrentFile')}
          >
            <Trash2 className="h-4 w-4" />
          </Button>
        )}
        <div className="flex-1" />
        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7 hover:bg-sidebar-accent/50"
          onClick={() => setHabitsOpen(true)}
          title={t('knowledge.habitsTitle')}
        >
          <Activity className="h-4 w-4" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7 hover:bg-sidebar-accent/50"
          onClick={() => setSettingsOpen(true)}
          title={t('knowledge.knowledgeSettings')}
        >
          <Settings className="h-4 w-4" />
        </Button>
        <span className="text-2xs text-sidebar-foreground/50 font-mono rounded border bg-sidebar/50 px-1.5 py-0.5">
          ⌘K
        </span>
      </div>
      <HabitsDialog open={habitsOpen} onOpenChange={setHabitsOpen} />
      <KnowledgeSettingsDialog open={settingsOpen} onOpenChange={setSettingsOpen} />
    </>
  )
}

// ── NavItemLink (shared) ───────────────────────────────────────

function NavItemLink({
  item,
  currentPath,
  collapsed,
}: {
  item: NavItem
  currentPath: string
  collapsed: boolean
}) {
  const { t } = useTranslation()
  const isActive =
    currentPath === item.href || (item.href !== '/' && currentPath.startsWith(item.href))
  const showBadge = item.badge != null && item.badge > 0

  const link = (
    <Link
      to={item.href}
      className={cn(itemBase, isActive ? itemActive : itemInactive, collapsed && 'justify-center')}
    >
      {item.icon}
      {!collapsed && <span>{t(item.labelKey)}</span>}
      {!collapsed && showBadge && (
        <span className="ml-auto flex h-4 min-w-4 items-center justify-center rounded-full bg-warning px-1 text-2xs font-bold text-white animate-scale-in">
          {item.badge}
        </span>
      )}
    </Link>
  )

  return collapsed ? (
    <Tooltip key={item.href}>
      <TooltipTrigger asChild>{link}</TooltipTrigger>
      <TooltipContent side="right">
        {`${t(item.labelKey)}${item.badge ? ` (${item.badge})` : ''}`}
      </TooltipContent>
    </Tooltip>
  ) : (
    <React.Fragment key={item.href}>{link}</React.Fragment>
  )
}
