// agent-panel — right-side container for the coding agent.
//
// Vertical regions (top to bottom):
//   1. Header — session title + status pill (running/idle).
//   2. ConversationView — message timeline (flex-1).
//   3. ReviewBar — collapsible pending-changes review (only when non-empty).
//   4. AgentInput — composer at the bottom.
//
// Renders empty-state copy when no session is active so the panel
// always has a presentable appearance.

import { Bot, Power, PowerOff } from 'lucide-react'
import { useCodeSessionStore } from '@/stores/code/code-session'
import { cn } from '@/lib/utils'
import { ConversationView } from './conversation-view'
import { AgentInput } from './agent-input'
import { ReviewBar } from '../review/review-bar'

export interface AgentPanelProps {
  /** Optional className for the outer wrapper. */
  className?: string
}

/**
 * AgentPanel — the right-hand conversation column. Wires together
 * the conversation view (top) and the input (bottom) and exposes a
 * minimal header with the active session title + a status pill.
 */
export function AgentPanel({ className }: AgentPanelProps) {
  const session = useCodeSessionStore((s) => s.session)
  const isAgentRunning = useCodeSessionStore((s) => s.isAgentRunning)

  return (
    <div
      className={cn(
        'flex flex-col h-full min-h-0 bg-surface text-foreground',
        className,
      )}
    >
      <header className="flex items-center gap-2 px-3 py-2 border-b border-line bg-surface">
        <Bot className="size-4 text-muted-foreground" />
        <div className="flex-1 min-w-0">
          <div className="text-sm font-medium truncate">
            {session?.title ?? 'Coding Agent'}
          </div>
          {session ? (
            <div className="text-[10px] text-muted-foreground truncate">
              {session.project_path}
            </div>
          ) : null}
        </div>
        <div
          className={cn(
            'flex items-center gap-1 rounded-full px-2 py-0.5 text-[10px] font-medium',
            isAgentRunning
              ? 'bg-emerald-500/10 text-emerald-600 dark:text-emerald-400'
              : 'bg-surface-sunken text-muted-foreground',
          )}
          aria-live="polite"
        >
          {isAgentRunning ? (
            <Power className="size-3" />
          ) : (
            <PowerOff className="size-3" />
          )}
          <span>{isAgentRunning ? 'Running' : 'Idle'}</span>
        </div>
      </header>

      <ConversationView />

      <ReviewBar />

      <AgentInput />
    </div>
  )
}
