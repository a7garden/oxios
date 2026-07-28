// CompressedGroup — controlled collapse toggle for older messages in long
// conversations (LobeHub analogue: Messages/CompressedGroup).
//
// The virtualized chat list (routes/chat.tsx) owns the expanded state and the
// row model: when collapsed, older messages are omitted from the VList and this
// bar is the first row; when expanded, all messages render as rows. This
// component is purely the toggle affordance — no internal state, no children.

import { ChevronDown, ChevronRight, MessagesSquare } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { cn } from '@/lib/utils'

interface CompressedGroupProps {
  /** Number of messages hidden while collapsed. */
  count: number
  expanded: boolean
  onToggle: () => void
  className?: string
}

export function CompressedGroup({ count, expanded, onToggle, className }: CompressedGroupProps) {
  const { t } = useTranslation()
  return (
    <button
      type="button"
      onClick={onToggle}
      className={cn(
        'flex w-full items-center gap-2 rounded-lg border border-dashed bg-muted/30 px-3 py-2 text-xs text-muted-foreground transition-colors hover:bg-muted/60',
        className,
      )}
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
  )
}
