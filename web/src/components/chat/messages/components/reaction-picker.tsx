// ReactionPicker — emoji reaction selector for messages.
//
// LobeHub analogue: Conversation/components/Reaction/. Oxios version is a
// lightweight popover with a fixed palette (no emoji search dependency).
// Reactions are localStorage-backed (see lib/reactions-storage) — ephemeral
// per browser, by design for the single-user desktop app.

import { SmilePlus } from 'lucide-react'
import { useCallback, useEffect, useRef, useState } from 'react'

const PALETTE = ['👍', '❤️', '🎉', '🤔', '👀', '🔥', '✅', '🙏']

interface ReactionPickerProps {
  /** Called when the user selects an emoji. */
  onSelect: (emoji: string) => void
  /** Optional anchor for the popover (defaults to left-aligned, top-right). */
  className?: string
}

export function ReactionPicker({ onSelect, className }: ReactionPickerProps) {
  const [open, setOpen] = useState(false)
  const ref = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!open) return
    const close = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false)
    }
    document.addEventListener('mousedown', close)
    return () => document.removeEventListener('mousedown', close)
  }, [open])

  const handleSelect = useCallback(
    (emoji: string) => {
      onSelect(emoji)
      setOpen(false)
    },
    [onSelect],
  )

  return (
    <div ref={ref} className={`relative ${className ?? ''}`}>
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className="flex h-7 w-7 items-center justify-center rounded text-muted-foreground hover:bg-muted hover:text-foreground transition-colors"
        aria-label="Add reaction"
      >
        <SmilePlus className="size-3.5" />
      </button>
      {open && (
        <div className="absolute z-20 right-0 top-full mt-1 flex gap-1 rounded-lg border bg-popover p-1 shadow-lg">
          {PALETTE.map((emoji) => (
            <button
              key={emoji}
              type="button"
              onClick={() => handleSelect(emoji)}
              className="flex h-7 w-7 items-center justify-center rounded text-base hover:bg-muted transition-colors"
            >
              {emoji}
            </button>
          ))}
        </div>
      )}
    </div>
  )
}
