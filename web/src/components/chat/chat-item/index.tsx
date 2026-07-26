// ChatItem — universal message wrapper (ported from LobeHub ChatItem)
//
// Provides: avatar, title bar (name + time), error display, message body,
// loading state, hover-revealed action bar, and follow-up space.
//
// LobeHub original: /tmp/lobehub/src/features/Conversation/ChatItem/ChatItem.tsx
// Dependencies removed: @lobehub/ui (Flexbox), antd-style (createStaticStyles, cx)
// Replaced with: Tailwind utility classes, cn() from clsx/tailwind-merge

import { Loader2 } from 'lucide-react'
import { memo } from 'react'
import { cn } from '@/lib/utils'
import type { ChatItemProps as _ChatItemProps, ChatError, ChatItemAvatar } from '@/types/chat'

// ── Re-export the props type ──
export type { ChatItemAvatar }
export type ChatItemProps = _ChatItemProps

// ── Sub-components ──

function TitleRow({
  name,
  time,
  durationMs,
}: {
  name?: string
  time?: number
  durationMs?: number
}) {
  return (
    <div className="flex items-center gap-2 mb-1">
      {name && <span className="text-sm font-medium">{name}</span>}
      {time != null && (
        <span className="text-xs text-muted-foreground">{formatChatTime(time)}</span>
      )}
      {durationMs != null && durationMs > 0 && (
        <span className="text-xs text-muted-foreground/70">· {formatDuration(durationMs)}</span>
      )}
    </div>
  )
}

function ErrorBlock({ error }: { error: ChatError }) {
  return (
    <div className="mb-2 px-3 py-2 rounded-md border border-destructive/50 bg-destructive/5 text-sm text-destructive">
      <p className="font-medium">{error.type}</p>
      {error.message && <p className="text-xs text-muted-foreground mt-0.5">{error.message}</p>}
    </div>
  )
}

function LoadingBlock() {
  return (
    <div className="flex items-center gap-2 text-sm text-muted-foreground py-1">
      <Loader2 className="w-3.5 h-3.5 animate-spin" />
      <span>...</span>
    </div>
  )
}

function ActionsBar({ children }: { children?: React.ReactNode }) {
  return (
    <div className="flex items-center gap-1 mt-1 opacity-0 group-hover:opacity-100 transition-opacity duration-150">
      {children}
    </div>
  )
}

// ── Main Component ──

export const ChatItem = memo(function ChatItem({
  id,
  avatar,
  placement = 'left',
  loading = false,
  error,
  time,
  durationMs,
  showTitle = true,
  actions,
  messageExtra,
  children,
  className,
}: ChatItemProps) {
  const isRight = placement === 'right'

  return (
    <div id={id} className={cn('group px-4 py-2', className)}>
      {/* No avatar column — user vs agent is distinguished by alignment
          (placement) plus a faint user tint. items-end right-aligns the
          user's content; the agent column stretches full-width. */}
      <div className={cn('flex flex-col min-w-0', isRight && 'items-end')}>
        {/* Title row — hidden until hover (assistant model name + time + duration) */}
        {showTitle && (
          <div className="opacity-0 group-hover:opacity-100 transition-opacity duration-150">
            <TitleRow name={avatar?.name} time={time} durationMs={durationMs} />
          </div>
        )}

        {/* Error display */}
        {error && <ErrorBlock error={error} />}

        {/* Loading or message body */}
        {loading ? <LoadingBlock /> : children}

        {/* Message extra (e.g. review outcome) */}
        {messageExtra && <div className="mt-1">{messageExtra}</div>}

        {/* Actions bar — hidden until hover */}
        <ActionsBar>{actions}</ActionsBar>
      </div>
    </div>
  )
})

// ── Helpers ──

function formatChatTime(ms: number): string {
  const d = new Date(ms)
  const now = new Date()
  const isToday =
    d.getDate() === now.getDate() &&
    d.getMonth() === now.getMonth() &&
    d.getFullYear() === now.getFullYear()

  const hh = d.getHours().toString().padStart(2, '0')
  const mm = d.getMinutes().toString().padStart(2, '0')

  if (isToday) return `${hh}:${mm}`
  return `${d.getMonth() + 1}/${d.getDate()} ${hh}:${mm}`
}

/** Compact turn-duration string: ms under 1s, "X.Xs" under 1m, else "Xm Ys". */
function formatDuration(ms: number): string {
  if (ms >= 60000) return `${Math.floor(ms / 60000)}m ${Math.round((ms % 60000) / 1000)}s`
  if (ms >= 1000) return `${(ms / 1000).toFixed(1)}s`
  return `${ms}ms`
}
