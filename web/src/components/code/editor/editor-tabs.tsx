import {
  CircleX,
  FileCode,
  FileJson,
  FileText,
  Hash,
  Image as ImageIcon,
  type LucideIcon,
} from 'lucide-react'
import type { EditorTab } from '@/types/code'
import { cn } from '@/lib/utils'
import { useCodeSessionStore } from '@/stores/code/code-session'

/** Pick a Lucide icon based on the file extension. Falls back to FileText. */
function fileIcon(name: string): LucideIcon {
  const ext = name.split('.').pop()?.toLowerCase() ?? ''
  if (['ts', 'tsx', 'js', 'jsx'].includes(ext)) return FileCode
  if (['json'].includes(ext)) return FileJson
  if (['md', 'markdown'].includes(ext)) return Hash
  if (['png', 'jpg', 'jpeg', 'gif', 'svg', 'webp', 'ico'].includes(ext)) return ImageIcon
  return FileText
}

interface TabButtonProps {
  tab: EditorTab
  active: boolean
  onSelect: (id: string) => void
  onClose: (id: string) => void
}

function TabButton({ tab, active, onSelect, onClose }: TabButtonProps) {
  const Icon = fileIcon(tab.name)

  return (
    <div
      role="tab"
      aria-selected={active}
      tabIndex={0}
      onClick={() => onSelect(tab.id)}
      onKeyDown={(e) => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault()
          onSelect(tab.id)
        } else if (e.key === 'Backspace' || (e.key === 'w' && (e.metaKey || e.ctrlKey))) {
          e.preventDefault()
          onClose(tab.id)
        }
      }}
      className={cn(
        'group relative flex h-full cursor-pointer items-center gap-1.5 border-r border-line bg-surface px-3 text-xs transition-colors outline-none',
        'focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-inset',
        active
          ? 'bg-surface text-foreground'
          : 'bg-surface-sunken text-muted-foreground hover:bg-surface hover:text-foreground',
      )}
    >
      {/* Bottom indicator for active tab */}
      {active && (
        <span className="pointer-events-none absolute inset-x-0 bottom-0 h-px bg-primary" />
      )}

      <Icon className="h-3.5 w-3.5 shrink-0" />

      <span className="max-w-[180px] truncate">{tab.name}</span>

      {/* Dirty indicator (filled dot) — takes precedence over the close button */}
      {tab.isDirty && (
        <span
          aria-label="Unsaved changes"
          className="ml-0.5 h-1.5 w-1.5 shrink-0 rounded-full bg-amber-500"
        />
      )}

      {/* Close button — hidden until hover unless dirty/active */}
      <button
        type="button"
        aria-label="Close tab"
        onClick={(e) => {
          e.stopPropagation()
          onClose(tab.id)
        }}
        className={cn(
          'ml-0.5 flex h-4 w-4 shrink-0 cursor-pointer items-center justify-center rounded-sm text-muted-foreground transition-colors hover:bg-line hover:text-foreground',
          tab.isDirty
            ? 'opacity-0 group-hover:opacity-100 focus-visible:opacity-100'
            : active
              ? 'opacity-0 group-hover:opacity-100 focus-visible:opacity-100'
              : 'opacity-0 group-hover:opacity-100 focus-visible:opacity-100',
        )}
      >
        <CircleX className="h-3.5 w-3.5" />
      </button>
    </div>
  )
}

/**
 * Tab bar for open editor files.
 *
 * Renders one TabButton per tab in `tabs`. Click selects the tab; the inline
 * close button removes it. Dirty tabs display an amber dot so unsaved state
 * is never hidden, even when the close button is hidden on idle tabs.
 */
export function EditorTabs() {
  const tabs = useCodeSessionStore((s) => s.tabs)
  const activeTabId = useCodeSessionStore((s) => s.activeTabId)
  const setActiveTab = useCodeSessionStore((s) => s.setActiveTab)
  const closeTab = useCodeSessionStore((s) => s.closeTab)

  if (tabs.length === 0) return null

  return (
    <div
      role="tablist"
      aria-label="Open editor tabs"
      className="flex h-9 shrink-0 items-stretch overflow-x-auto border-b bg-surface-sunken"
    >
      {tabs.map((tab) => (
        <TabButton
          key={tab.id}
          tab={tab}
          active={tab.id === activeTabId}
          onSelect={setActiveTab}
          onClose={closeTab}
        />
      ))}
    </div>
  )
}