// CompressedGroup — collapsible wrapper for older messages in long conversations.
//
// LobeHub analogue: Messages/CompressedGroup (summary/history tabs, expand/collapse).
// Oxios version: when a conversation exceeds COLLAPSE_THRESHOLD messages, the
// oldest messages (beyond VISIBLE_TAIL) are grouped into this collapsible bar.
// Clicking expands to reveal the full history inline.

import { ChevronDown, ChevronRight, MessagesSquare } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { cn } from '@/lib/utils'

interface CompressedGroupProps {
  /** Number of messages hidden inside the collapsed group. */
  count: number
  /** The collapsed message elements (rendered only when expanded). */
  children: React.ReactNode
  className?: string
}

export function CompressedGroup({ count, children, className }: CompressedGroupProps) {
  const { t } = useTranslation()
  const [expanded, setExpanded] = useState(false)

  return (
    <div className={cn('', className)}>
      <button
        type="button"
        onClick={() => setExpanded((v) => !v)}
        className="flex w-full items-center gap-2 rounded-lg border border-dashed bg-muted/30 px-3 py-2 text-xs text-muted-foreground transition-colors hover:bg-muted/60"
      >
        {expanded ? (
          <ChevronDown className="size-3.5 shrink-0" />
        ) : (
          <ChevronRight className="size-3.5 shrink-0" />
        )}
        <MessagesSquare className="size-3.5 shrink-0" />
        <span>
          {expanded ? t('chat.compressedExpanded') : t('chat.compressedCollapsed', { count })}
        </span>
      </button>
      {expanded && <div className="mt-1 space-y-1 opacity-70">{children}</div>}
    </div>
  )
}
