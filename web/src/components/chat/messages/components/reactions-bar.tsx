// ReactionsBar — displays existing emoji reactions on a message + the
// ReactionPicker trigger. Toggling an existing reaction re-toggles it.

import { useCallback } from 'react'
import { listReactions, toggleReaction } from '@/lib/reactions-storage'
import { cn } from '@/lib/utils'
import { ReactionPicker } from './reaction-picker'

interface ReactionsBarProps {
  messageId: string
  /** Force re-render when localStorage changes (parent provides a version counter). */
  version: number
  className?: string
}

export function ReactionsBar({ messageId, version, className }: ReactionsBarProps) {
  const handleSelect = useCallback(
    (emoji: string) => {
      toggleReaction(messageId, emoji)
      // Trigger parent re-render via a custom event since we don't own the state.
      window.dispatchEvent(new CustomEvent('reactions-changed'))
    },
    [messageId],
  )

  const handleToggleExisting = useCallback(
    (emoji: string) => {
      toggleReaction(messageId, emoji)
      window.dispatchEvent(new CustomEvent('reactions-changed'))
    },
    [messageId],
  )

  // `version` is consumed here so the parent re-render propagates to this bar.
  void version
  const reactions = listReactions(messageId)

  if (reactions.length === 0) {
    return (
      <div className={cn('flex items-center gap-1', className)}>
        <ReactionPicker onSelect={handleSelect} />
      </div>
    )
  }

  return (
    <div className={cn('flex flex-wrap items-center gap-1', className)}>
      {reactions.map(({ emoji }) => (
        <button
          key={emoji}
          type="button"
          onClick={() => handleToggleExisting(emoji)}
          className="flex items-center gap-1 rounded-full border bg-muted/40 px-2 py-0.5 text-xs transition-colors hover:bg-muted"
          aria-label={`Toggle reaction ${emoji}`}
        >
          <span>{emoji}</span>
        </button>
      ))}
      <ReactionPicker onSelect={handleSelect} />
    </div>
  )
}
