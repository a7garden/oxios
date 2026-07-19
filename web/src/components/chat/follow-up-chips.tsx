// FollowUpChips — AI-suggested follow-up questions after assistant messages
// Ported from LobeHub's FollowUpChips pattern.
// When enabled in system agent config, generates 3 clickable suggestions.

'use client'

import { useQuery } from '@tanstack/react-query'
import { Lightbulb } from 'lucide-react'
import { api } from '@/lib/api-client'
import { cn } from '@/lib/utils'

// ── Types ──

interface FollowUpSuggestions {
  suggestions: string[]
}

// ── Props ──

interface FollowUpChipsProps {
  /** Session ID for context. */
  sessionId?: string
  /** Message ID of the last assistant response. */
  messageId?: string
  /** Message content for heuristic fallback. */
  content: string
  /** Click handler when a chip is selected. */
  onSelect: (suggestion: string) => void
  className?: string
}

// ── Component ──

export function FollowUpChips({
  sessionId,
  messageId,
  content,
  onSelect,
  className,
}: FollowUpChipsProps) {
  // Try to fetch AI-generated suggestions
  const { data: aiSuggestions } = useQuery({
    queryKey: ['follow-up', sessionId, messageId],
    queryFn: () =>
      api.post<FollowUpSuggestions>('/api/engine/follow-up', {
        session_id: sessionId,
        message_id: messageId,
      }),
    enabled: false, // Disabled until backend API exists — using heuristic fallback
    retry: false,
  })

  // Heuristic fallback: generate suggestions from content
  const suggestions = aiSuggestions?.suggestions ?? generateHeuristicSuggestions(content)

  if (suggestions.length === 0) return null

  return (
    <div className={cn('flex flex-wrap gap-1.5 mt-2', className)}>
      {suggestions.map((suggestion, i) => (
        <button
          key={i}
          type="button"
          onClick={() => onSelect(suggestion)}
          className="group inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full border bg-card text-xs text-muted-foreground hover:border-primary/30 hover:text-foreground transition-all"
        >
          <Lightbulb className="w-3 h-3 text-amber-500/70 group-hover:text-amber-500 transition-colors shrink-0" />
          <span className="truncate max-w-[240px]">{suggestion}</span>
        </button>
      ))}
    </div>
  )
}

// ── Heuristic suggestion generator ──

function generateHeuristicSuggestions(content: string): string[] {
  const suggestions: string[] = []
  const lower = content.toLowerCase()

  // Code-related
  if (lower.includes('```') || lower.includes('function') || lower.includes('class')) {
    suggestions.push('이 코드를 어떻게 개선할 수 있을까?')
  }

  // List/steps
  if (lower.includes('1.') || lower.includes('step') || lower.includes('단계')) {
    suggestions.push('각 단계를 더 자세히 설명해줘')
  }

  // Questions in content
  const questions = content.match(/[^.?!]*\?/g)
  if (questions && questions.length > 0) {
    suggestions.push('그 부분에 대해 더 설명해줘')
  }

  // Comparison
  if (lower.includes('vs') || lower.includes('비교') || lower.includes('차이')) {
    suggestions.push('어떤 걸 선택해야 할까?')
  }

  // Error/troubleshooting
  if (lower.includes('error') || lower.includes('오류') || lower.includes('문제')) {
    suggestions.push('다른 해결 방법도 있어?')
  }

  // Default suggestions if nothing matched
  if (suggestions.length === 0) {
    suggestions.push('더 자세히 알려줘')
    suggestions.push('예시를 들어줘')
  }

  // Always add a "continue" option if content seems incomplete
  if (content.length > 500 && !content.endsWith('.')) {
    suggestions.unshift('계속해줘')
  }

  // Cap at 3 suggestions
  return suggestions.slice(0, 3)
}
