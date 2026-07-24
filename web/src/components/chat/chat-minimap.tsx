// ChatMiniMap — hover-reveal vertical overview rail for the message list.
//
// LobeHub analogue: features/ChatMiniMap (narrow right-edge indicator with
// per-message dots that expand into a peek panel on hover; clicking a marker
// scrolls the message into view).
//
// Mounted by the chat route as a sibling of the scrollable messages column
// (positioned absolute, right-0, full height of the column). Hidden when
// there are fewer than MIN_MESSAGES_THRESHOLD messages — the affordance
// only earns its keep on long sessions.

import { useTranslation } from 'react-i18next'
import { cn } from '@/lib/utils'
import type { ChatMessage } from '@/types'

/** Below this many messages the minimap is hidden — it adds visual noise
 *  for short threads and the scroll position is already easy to read. */
const MIN_MESSAGES_THRESHOLD = 20
/** Width of the always-visible (collapsed) rail. */
const RAIL_WIDTH = 8

/** Number of characters of the message body to show in the peek preview. */
const PREVIEW_CHARS = 50
interface ChatMiniMapProps {
  messages: ChatMessage[]
  /** Called with the message index when the user clicks a marker. The parent
   *  is responsible for scrolling that message into view. */
  onJump?: (index: number) => void
  className?: string
}

/** Color token for each role. Matches the chat surface's role accents. */
function roleColor(role: ChatMessage['role']): string {
  switch (role) {
    case 'user':
      return 'bg-blue-500'
    case 'assistant':
      return 'bg-emerald-500'
    case 'tool':
      return 'bg-gray-400'
    default:
      // system / unknown — kept neutral so the user can still see position.
      return 'bg-gray-300'
  }
}

/** Strip whitespace + first PREVIEW_CHARS, collapse line breaks. */
function previewText(content: string): string {
  const flat = content.replace(/\s+/g, ' ').trim()
  if (flat.length <= PREVIEW_CHARS) return flat
  return `${flat.slice(0, PREVIEW_CHARS)}…`
}

export function ChatMiniMap({ messages, onJump, className }: ChatMiniMapProps) {
  const { t } = useTranslation()

  if (messages.length <= MIN_MESSAGES_THRESHOLD) return null

  return (
    <nav
      data-testid="chat-minimap"
      aria-label={t('chat.minimap.ariaLabel')}
      // Outer container: a thin column on the right that expands on hover.
      // Using width transitions keeps the collapsed rail narrow and
      // predictable while the preview panel animates in.
      className={cn(
        'group absolute right-0 top-0 bottom-0 z-5 flex flex-col items-stretch overflow-hidden border-l bg-background/60 backdrop-blur-sm transition-[width] duration-200 ease-out',
        'hover:w-[200px] focus-within:w-[200px]',
        className,
      )}
      style={{ width: RAIL_WIDTH }}
    >
      {/* Marker column. `flex-1` rows + `gap-px` keep dots evenly stacked. */}
      <ul className="flex h-full w-full flex-col items-stretch justify-between gap-px p-1">
        {messages.map((msg, i) => {
          const preview = previewText(msg.content ?? '')
          return (
            <li
              key={msg.id ?? i}
              // Each row: a coloured segment. Hover on the row highlights
              // the dot and shows its inline preview chip.
              className="group/marker relative flex-1"
            >
              <button
                type="button"
                onClick={() => onJump?.(i)}
                title={preview}
                aria-label={t('chat.minimap.jumpToMessage', { index: i + 1 })}
                className={cn(
                  'flex h-full w-full items-center justify-start rounded-sm transition-all duration-150',
                  'hover:bg-foreground/5',
                )}
              >
                <span
                  aria-hidden
                  className={cn(
                    'block h-full w-1.5 rounded-sm transition-all duration-150',
                    roleColor(msg.role),
                    'group-hover/marker:w-2 group-hover/marker:shadow-sm',
                  )}
                />
                {/* Preview chip: anchored to the left of the dot, revealed
                    on per-marker hover. Rendered per marker so it lines up
                    with the row regardless of scroll position. */}
                <span
                  className={cn(
                    'pointer-events-none absolute right-full top-1/2 -translate-y-1/2 me-1.5 hidden max-w-[180px] truncate rounded border bg-popover px-2 py-0.5 text-[10px] text-popover-foreground shadow-sm',
                    'group-hover/marker:inline-block',
                  )}
                >
                  {preview || t('chat.minimap.emptyMessage')}
                </span>
              </button>
            </li>
          )
        })}
      </ul>
    </nav>
  )
}
