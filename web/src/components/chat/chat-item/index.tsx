// ChatItem — universal message wrapper (ported from LobeHub ChatItem)
//
// Provides: avatar, title bar (name + time), error display, message body,
// loading state, hover-revealed action bar, and follow-up space.
//
// LobeHub original: /tmp/lobehub/src/features/Conversation/ChatItem/ChatItem.tsx
// Dependencies removed: @lobehub/ui (Flexbox), antd-style (createStaticStyles, cx)
// Replaced with: Tailwind utility classes, cn() from clsx/tailwind-merge

import { cn } from '@/lib/utils'
import type { ChatError, ChatItemAvatar, ChatItemProps as _ChatItemProps } from '@/types/chat'
import { Loader2 } from 'lucide-react'
import { memo } from 'react'

// ── Re-export the props type ──
export type { ChatItemAvatar }
export type ChatItemProps = _ChatItemProps

// ── Sub-components ──

function Avatar({ name, avatar, color }: ChatItemAvatar) {
  if (avatar) {
    return (
      <img
        src={avatar}
        alt={name ?? 'agent'}
        className="w-7 h-7 rounded-full shrink-0 mt-1"
      />
    )
  }
  // Fallback: initials circle
  const fallbackName = name ?? '?'
  const initial = fallbackName.charAt(0).toUpperCase()
  const bg = color ?? 'bg-muted'
  return (
    <div
      className={cn(
        'w-7 h-7 rounded-full shrink-0 mt-1 flex items-center justify-center text-xs font-semibold',
        bg,
        'text-muted-foreground',
      )}
    >
      {initial}
    </div>
  )
}

function TitleRow({
  name,
  time,
}: {
  name?: string
  time?: number
}) {
  return (
    <div className="flex items-center gap-2 mb-1">
      {name && <span className="text-sm font-medium">{name}</span>}
      {time != null && (
        <span className="text-xs text-muted-foreground">
          {formatChatTime(time)}
        </span>
      )}
    </div>
  )
}

function ErrorBlock({ error }: { error: ChatError }) {
  return (
    <div className="mb-2 px-3 py-2 rounded-md border border-destructive/50 bg-destructive/5 text-sm text-destructive">
      <p className="font-medium">{error.type}</p>
      {error.message && (
        <p className="text-xs text-muted-foreground mt-0.5">{error.message}</p>
      )}
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
  showTitle = true,
  showAvatar = true,
  actions,
  messageExtra,
  children,
  className,
}: ChatItemProps) {
  const isRight = placement === 'right'

  return (
    <div
      id={id}
      className={cn(
        'group flex gap-3 px-4 py-2',
        isRight && 'flex-row-reverse',
        className,
      )}
    >
      {/* Avatar column */}
      {showAvatar ? (
        <Avatar {...avatar} />
      ) : (
        <div className="w-7 shrink-0" /> // spacer to keep alignment
      )}

      {/* Content column */}
      <div className="flex-1 min-w-0">
        {/* Title row — hidden until hover */}
        {showTitle && (
          <div className="opacity-0 group-hover:opacity-100 transition-opacity duration-150">
            <TitleRow name={avatar.name} time={time} />
          </div>
        )}

        {/* Error display */}
        {error && <ErrorBlock error={error} />}

        {/* Loading or message body */}
        {loading ? <LoadingBlock /> : children}

        {/* Message extra (e.g. usage stats) */}
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
