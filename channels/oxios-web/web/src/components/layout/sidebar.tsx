import { Link, useRouterState } from '@tanstack/react-router'
import {
  Activity,
  Bell,
  Bot,
  Brain,
  Calendar,
  CheckSquare,
  Cpu,
  Dna,
  FolderOpen,
  GitBranch,
  LayoutDashboard,
  MessageSquare,
  Monitor,
  Moon,
  NotebookPen,
  PanelLeft,
  PanelLeftClose,
  Settings,
  Shield,
  Sun,
  Timer,
  Theater,
  UsersRound,
  Wallet,
  Wrench,
  Zap,
} from 'lucide-react'
import { useQuery } from '@tanstack/react-query'
import { Separator } from '@/components/ui/separator'
import { Tooltip } from '@/components/ui/tooltip'
import { cn } from '@/lib/utils'
import { useSidebarStore } from '@/stores/sidebar'
import { useThemeStore } from '@/stores/theme'
import { api } from '@/lib/api-client'

interface NavItem {
  label: string
  href: string
  icon: React.ReactNode
  /** Only show this item when condition is true. Always visible when omitted. */
  show?: boolean
  /** Optional badge content (e.g. pending count). */
  badge?: number
}

const navGroups: { label: string; items: NavItem[] }[] = [
  {
    label: 'Main',
    items: [
      { label: 'Dashboard', href: '/', icon: <LayoutDashboard className="h-4 w-4" /> },
      { label: 'Chat', href: '/chat', icon: <MessageSquare className="h-4 w-4" /> },
    ],
  },
  {
    label: 'Agents',
    items: [
      { label: 'Agents', href: '/agents', icon: <Bot className="h-4 w-4" /> },
      { label: 'Seeds', href: '/seeds', icon: <Dna className="h-4 w-4" /> },
      { label: 'Groups', href: '/agent-groups', icon: <UsersRound className="h-4 w-4" /> },
      { label: 'Personas', href: '/personas', icon: <Theater className="h-4 w-4" /> },
      { label: 'Programs', href: '/programs', icon: <Cpu className="h-4 w-4" /> },
      { label: 'Skills', href: '/skills', icon: <Zap className="h-4 w-4" /> },
    ],
  },
  {
    label: 'Storage',
    items: [
      { label: 'Knowledge', href: '/knowledge', icon: <NotebookPen className="h-4 w-4" /> },
      { label: 'Memory', href: '/memory', icon: <Brain className="h-4 w-4" /> },
      { label: 'Workspace', href: '/workspace', icon: <FolderOpen className="h-4 w-4" /> },
    ],
  },
  {
    label: 'Monitor',
    items: [
      { label: 'Resources', href: '/resources', icon: <Activity className="h-4 w-4" /> },
      { label: 'Scheduler', href: '/scheduler', icon: <Calendar className="h-4 w-4" /> },
      { label: 'Cron Jobs', href: '/cron-jobs', icon: <Timer className="h-4 w-4" /> },
      { label: 'Budget', href: '/budget', icon: <Wallet className="h-4 w-4" /> },
      { label: 'Security', href: '/security', icon: <Shield className="h-4 w-4" /> },
      { label: 'Events', href: '/events', icon: <Bell className="h-4 w-4" /> },
      { label: 'Git', href: '/git', icon: <GitBranch className="h-4 w-4" /> },
      { label: 'Host Tools', href: '/host-tools', icon: <Wrench className="h-4 w-4" /> },
    ],
  },
]

/** Dynamic items that appear conditionally at the top of Main. */
function useDynamicItems(): NavItem[] {
  const { data } = useQuery({
    queryKey: ['approvals-pending-count'],
    queryFn: async () => {
      const res = await api.get<{ id: string; status: string }[]>('/api/approvals')
      const items = Array.isArray(res) ? res : []
      return items.filter((a) => a.status === 'pending').length
    },
    refetchInterval: 10_000,
  })

  const count = data ?? 0
  return [
    {
      label: 'Approvals',
      href: '/approvals',
      icon: <CheckSquare className="h-4 w-4" />,
      show: count > 0,
      badge: count,
    },
  ]
}

export function Sidebar() {
  const { collapsed, toggle } = useSidebarStore()
  const { theme, resolved, setTheme } = useThemeStore()
  const router = useRouterState()
  const currentPath = router.location.pathname
  const dynamicItems = useDynamicItems()

  // Insert dynamic items after the first static Main item (Dashboard)
  const mainGroup = navGroups[0]!
  const mainItems = [
    mainGroup.items[0]!, // Dashboard
    ...dynamicItems.filter((d) => d.show !== false),
    mainGroup.items[1]!, // Chat
  ]

  return (
    <aside
      className={cn(
        'flex flex-col border-r bg-sidebar text-sidebar-foreground transition-all duration-300',
        collapsed ? 'w-16' : 'w-60',
      )}
    >
      {/* Header */}
      <div className="flex h-14 items-center justify-between px-3">
        {!collapsed && (
          <div className="flex items-center gap-2">
            <Zap className="h-5 w-5 text-primary" />
            <span className="font-bold text-lg">Oxios</span>
          </div>
        )}
        <button
          type="button"
          onClick={toggle}
          className="rounded-md p-1.5 hover:bg-sidebar-accent"
          aria-label={collapsed ? 'Expand sidebar' : 'Collapse sidebar'}
        >
          {collapsed ? <PanelLeft className="h-4 w-4" /> : <PanelLeftClose className="h-4 w-4" />}
        </button>
      </div>
      <Separator />

      {/* Nav */}
      <nav className="flex-1 overflow-y-auto p-2">
        {/* Main group with dynamic approvals */}
        <div className="mb-3">
          {!collapsed && (
            <p className="mb-1 px-2 text-xs font-medium text-muted-foreground uppercase tracking-wider">
              Main
            </p>
          )}
          {mainItems.map((item) => {
            const isActive =
              currentPath === item.href ||
              (item.href !== '/' && currentPath.startsWith(item.href))
            const link = (
              <Link
                key={item.href}
                to={item.href}
                className={cn(
                  'flex items-center gap-3 rounded-lg px-2.5 py-2 text-sm transition-colors',
                  isActive
                    ? 'bg-sidebar-accent text-sidebar-accent-foreground font-medium'
                    : 'text-sidebar-foreground/70 hover:bg-sidebar-accent/50 hover:text-sidebar-foreground',
                  collapsed && 'justify-center',
                )}
              >
                {item.icon}
                {!collapsed && <span>{item.label}</span>}
                {!collapsed && item.badge != null && item.badge > 0 && (
                  <span className="ml-auto flex h-4 min-w-4 items-center justify-center rounded-full bg-amber-500 px-1 text-[10px] font-bold text-white">
                    {item.badge}
                  </span>
                )}
              </Link>
            )
            return collapsed ? (
              <Tooltip key={item.href} content={`${item.label}${item.badge ? ` (${item.badge})` : ''}`} side="right">
                {link}
              </Tooltip>
            ) : (
              link
            )
          })}
        </div>

        {/* Remaining static groups */}
        {navGroups.slice(1).map((group) => (
          <div key={group.label} className="mb-3">
            {!collapsed && (
              <p className="mb-1 px-2 text-xs font-medium text-muted-foreground uppercase tracking-wider">
                {group.label}
              </p>
            )}
            {group.items.map((item) => {
              const isActive =
                currentPath === item.href ||
                (item.href !== '/' && currentPath.startsWith(item.href))
              const link = (
                <Link
                  key={item.href}
                  to={item.href}
                  className={cn(
                    'flex items-center gap-3 rounded-lg px-2.5 py-2 text-sm transition-colors',
                    isActive
                      ? 'bg-sidebar-accent text-sidebar-accent-foreground font-medium'
                      : 'text-sidebar-foreground/70 hover:bg-sidebar-accent/50 hover:text-sidebar-foreground',
                    collapsed && 'justify-center',
                  )}
                >
                  {item.icon}
                  {!collapsed && <span>{item.label}</span>}
                </Link>
              )
              return collapsed ? (
                <Tooltip key={item.href} content={item.label} side="right">
                  {link}
                </Tooltip>
              ) : (
                link
              )
            })}
          </div>
        ))}
      </nav>

      <Separator />
      {/* Footer */}
      <div className="p-2 flex flex-col gap-1">
        <button
          type="button"
          onClick={() => {
            const next = theme === 'dark' ? 'light' : theme === 'light' ? 'system' : 'dark'
            setTheme(next)
          }}
          className="flex items-center gap-3 rounded-lg px-2.5 py-2 text-sm w-full hover:bg-sidebar-accent/50"
          aria-label="Toggle theme"
        >
          {theme === 'system' ? (
            <Monitor className="h-4 w-4" />
          ) : resolved === 'dark' ? (
            <Sun className="h-4 w-4" />
          ) : (
            <Moon className="h-4 w-4" />
          )}
          {!collapsed && (
            <span>{theme === 'system' ? 'System' : resolved === 'dark' ? 'Light' : 'Dark'}</span>
          )}
        </button>
        <Link
          to="/settings"
          className="flex items-center gap-3 rounded-lg px-2.5 py-2 text-sm hover:bg-sidebar-accent/50"
        >
          <Settings className="h-4 w-4" />
          {!collapsed && <span>Settings</span>}
        </Link>
      </div>
    </aside>
  )
}
