import { Bot } from 'lucide-react'
import { useTranslation } from 'react-i18next'

interface EmptyChatStateProps {
  /** Called when a user clicks a suggestion chip. Receives the suggestion text. */
  onSuggestionClick: (text: string) => void
}

interface Suggestion {
  emoji: string
  text: string
}

/**
 * Empty state shown when the chat has no messages yet. Displays a greeting
 * and a grid of suggestion chips the user can click to populate the input.
 *
 * Clicking a chip does NOT auto-send — it only fills the input so the user
 * can review/edit before sending. This matches Claude.ai's behavior.
 */
export function EmptyChatState({ onSuggestionClick }: EmptyChatStateProps) {
  const { t } = useTranslation()
  const suggestions: Suggestion[] = [
    { emoji: '📝', text: t('chat.suggestions.codeReview', 'Review my recent code changes') },
    { emoji: '🔍', text: t('chat.suggestions.techTrend', "What's trending in AI this week?") },
    {
      emoji: '📁',
      text: t('chat.suggestions.projectAnalysis', 'Summarize what this project does'),
    },
    { emoji: '🗓️', text: t('chat.suggestions.todaySchedule', "What's on my schedule today?") },
  ]

  return (
    <div className="flex flex-col items-center justify-center h-full gap-6 px-4 text-muted-foreground">
      <div className="text-center">
        <Bot className="h-12 w-12 mx-auto mb-3 text-primary/60" />
        <p className="text-base font-medium text-foreground">
          {t('chat.greeting', 'What can I help you with?')}
        </p>
        <p className="text-xs mt-1">{t('chat.greetingHint', 'Pick a suggestion or type your own.')}</p>
      </div>
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-2 w-full max-w-lg">
        {suggestions.map((s) => (
          <button
            key={s.text}
            type="button"
            onClick={() => onSuggestionClick(s.text)}
            className="flex items-center gap-2.5 px-4 py-3 rounded-lg border bg-card hover:bg-accent/50 hover:border-primary/30 transition-colors text-sm text-left text-foreground"
          >
            <span className="text-lg shrink-0">{s.emoji}</span>
            <span className="truncate">{s.text}</span>
          </button>
        ))}
      </div>
    </div>
  )
}
