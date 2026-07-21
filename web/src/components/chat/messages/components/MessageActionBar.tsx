// MessageActionBar — reusable hover-revealed action bar for any message role.
//
// Renders a compact row of icon buttons. Each action has: icon (React node),
// label (for title/aria), onClick, optional `danger` flag for destructive
// styling, optional `hidden` to skip rendering.
//
// Used by AssistantMessage (copy/regenerate/retry/delete) and UserMessage
// (edit/delete). LobeHub analogue: Messages/components/MessageActionBar.
//
// Hover-reveal is handled by ChatItem's ActionsBar wrapper — this component
// is just the button row itself.

import type { ReactNode } from 'react'
import { cn } from '@/lib/utils'

export interface MessageAction {
  id: string
  icon: ReactNode
  label: string
  onClick: () => void
  danger?: boolean
  hidden?: boolean
  /** Override button content (e.g. transient 'Copied' text). */
  children?: ReactNode
}

interface MessageActionBarProps {
  actions: MessageAction[]
  className?: string
}

export function MessageActionBar({ actions, className }: MessageActionBarProps) {
  const visible = actions.filter((a) => !a.hidden)
  if (visible.length === 0) return null
  return (
    <div className={cn('flex items-center gap-0.5', className)}>
      {visible.map((a) => (
        <button
          key={a.id}
          type="button"
          onClick={a.onClick}
          title={a.label}
          aria-label={a.label}
          className={cn(
            'inline-flex items-center justify-center w-7 h-7 rounded text-xs text-muted-foreground hover:bg-muted transition-colors',
            a.danger ? 'hover:text-destructive' : 'hover:text-foreground',
          )}
        >
          {a.children ?? a.icon}
        </button>
      ))}
    </div>
  )
}
