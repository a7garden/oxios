import { Bot, Power } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Handle, type NodeProps, Position } from 'reactflow'
import { statusBorder, statusDot } from '@/components/shared/status-palette'
import { cn } from '@/lib/utils'
import type { TopologyNode } from '@/types/a2a'

/** Human-readable "X seconds ago" / "X minutes ago" formatter. */
function formatLastSeen(
  iso: string | null,
  t: (k: string, opts?: { count?: number }) => string,
): string {
  if (!iso) return t('a2a.neverSeen')
  const ts = new Date(iso).getTime()
  if (Number.isNaN(ts)) return t('a2a.neverSeen')
  const diffSec = Math.max(0, Math.floor((Date.now() - ts) / 1000))
  if (diffSec < 60) return t('a2a.lastSeenSeconds', { count: diffSec })
  const min = Math.floor(diffSec / 60)
  if (min < 60) return t('a2a.lastSeenMinutes', { count: min })
  const hr = Math.floor(min / 60)
  if (hr < 24) return t('a2a.lastSeenHours', { count: hr })
  const day = Math.floor(hr / 24)
  return t('a2a.lastSeenDays', { count: day })
}

export interface AgentNodeData extends TopologyNode {
  /** Optional click handler so the parent can open the inspector. */
  onSelect?: (id: string) => void
  /** True when the node is the currently selected one. */
  selected?: boolean
}

/**
 * Custom React Flow node for an A2A agent.
 *
 * Shows the agent name, a colored status dot, a count of
 * capabilities and skills, and the last-seen time. Source and
 * target handles are on the left/right so edges flow horizontally.
 */
export function AgentNode({ data, id }: NodeProps<AgentNodeData>) {
  const { t } = useTranslation()
  const borderClass = statusBorder(data.status)
  const dotClass = statusDot(data.status)

  return (
    <button
      type="button"
      className={cn(
        'min-w-[200px] max-w-[260px] rounded-lg border-2 bg-card p-3 shadow-sm transition focus:outline-none focus:ring-2 focus:ring-ring text-left',
        borderClass,
        data.selected && 'ring-2 ring-ring',
      )}
      aria-label={t('a2a.nodeAriaLabel', { name: data.label, status: data.status })}
      onKeyDown={(e) => {
        if ((e.key === 'Enter' || e.key === ' ') && data.onSelect) {
          e.preventDefault()
          data.onSelect(id)
        }
      }}
      onClick={() => data.onSelect?.(id)}
    >
      <Handle type="target" position={Position.Left} className="!bg-muted-foreground" />
      <div className="flex items-center gap-2">
        <Bot className="h-4 w-4 text-foreground" aria-hidden="true" />
        <span className="font-medium text-sm text-foreground truncate">{data.label}</span>
        <span
          className={cn('ml-auto h-2 w-2 rounded-full', dotClass)}
          aria-hidden="true"
          title={data.status}
        />
      </div>

      <div className="text-xs text-muted-foreground mt-1.5">
        <span>{t('a2a.capabilitiesCount', { count: data.capabilities?.length ?? 0 })}</span>
        <span className="mx-1.5">·</span>
        <span>{t('a2a.skillsCount', { count: data.skills?.length ?? 0 })}</span>
      </div>

      <div className="text-2xs text-muted-foreground/80 mt-0.5 flex items-center gap-1">
        <Power className="h-2.5 w-2.5" aria-hidden="true" />
        {formatLastSeen(data.last_seen, t)}
      </div>
      <Handle type="source" position={Position.Right} className="!bg-muted-foreground" />
    </button>
  )
}
