// FollowUpChips — context-aware follow-up suggestions after assistant messages.
//
// Ported from LobeHub's FollowUpChips pattern. When the assistant finishes
// responding, the last message text is sent to /api/engine/follow-up, which
// runs a lightweight LLM "sidecar" call that extracts 0-4 clickable reply
// chips. Chips are language-matched to the message and context-aware.

import { useQuery } from '@tanstack/react-query'
import { Lightbulb } from 'lucide-react'
import { api } from '@/lib/api-client'
import { cn } from '@/lib/utils'

// ── Types ──

interface FollowUpChip {
  /** Short label shown on the chip (≤40 chars). */
  label: string
  /** Full message text sent on click (≤200 chars). May equal label. */
  message: string
}

interface FollowUpSuggestions {
  suggestions: FollowUpChip[]
}

// ── Props ──

interface FollowUpChipsProps {
  /** Message content for the LLM to extract suggestions from. */
  content: string
  /** Whether the message is still streaming — suppresses the query until done. */
  generating?: boolean
  /** Click handler when a chip is selected. */
  onSelect: (message: string) => void
  className?: string
}

// ── Component ──

export function FollowUpChips({ content, generating, onSelect, className }: FollowUpChipsProps) {
  // Fire the AI suggestion query once the message is complete.
  // Using `content` as the key means: same content → cached, regenerated → refetch.
  const { data } = useQuery({
    queryKey: ['follow-up', content],
    queryFn: () => api.post<FollowUpSuggestions>('/api/engine/follow-up', { content }),
    enabled: !generating && content.length > 0,
    staleTime: Infinity,
    retry: false,
    gcTime: 5 * 60 * 1000,
  })

  const chips = data?.suggestions ?? []

  if (chips.length === 0) return null

  return (
    <div className={cn('flex flex-wrap gap-1.5 mt-2', className)}>
      {chips.map((chip, i) => (
        <button
          key={i}
          type="button"
          onClick={() => onSelect(chip.message)}
          className="group inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full border bg-card text-xs text-muted-foreground hover:border-primary/30 hover:text-foreground transition-all"
          style={{ animationDelay: `${i * 60}ms` }}
        >
          <Lightbulb className="w-3 h-3 text-amber-500/70 group-hover:text-amber-500 transition-colors shrink-0" />
          <span className="truncate max-w-[240px]">{chip.label}</span>
        </button>
      ))}
    </div>
  )
}
