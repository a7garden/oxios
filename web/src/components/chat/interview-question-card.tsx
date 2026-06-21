import { useTranslation } from 'react-i18next'
import { cn } from '@/lib/utils'
import type { InterviewQuestion } from '@/types'

interface InterviewQuestionCardProps {
  question: InterviewQuestion
  value: string | string[] | undefined
  onChange: (value: string | string[]) => void
  disabled?: boolean
}

/**
 * Renders a single interview question with the appropriate widget
 * based on `question.kind`: single_choice (chips), multi_choice (toggleable
 * chips), yes_no (binary buttons), free_text (textarea).
 */
export function InterviewQuestionCard({
  question,
  value,
  onChange,
  disabled,
}: InterviewQuestionCardProps) {
  const { kind, text, options } = question
  const { t } = useTranslation()

  if (kind === 'single_choice') {
    return (
      <div>
        <p className="text-sm font-medium mb-2">{text}</p>
        <div className="flex flex-wrap gap-2">
          {options?.map((opt) => (
            <button
              key={opt.value}
              type="button"
              onClick={() => onChange(opt.value)}
              disabled={disabled}
              className={cn(
                'px-3 py-1.5 rounded-full text-sm border transition-colors',
                value === opt.value
                  ? 'bg-primary text-primary-foreground border-primary'
                  : 'bg-card hover:bg-accent/50 border-border',
              )}
            >
              {opt.label}
            </button>
          ))}
        </div>
      </div>
    )
  }

  if (kind === 'multi_choice') {
    const selected = Array.isArray(value) ? value : []
    return (
      <div>
        <p className="text-sm font-medium mb-2">{text}</p>
        <div className="flex flex-wrap gap-2">
          {options?.map((opt) => {
            const isActive = selected.includes(opt.value)
            return (
              <button
                key={opt.value}
                type="button"
                onClick={() => {
                  onChange(
                    isActive ? selected.filter((v) => v !== opt.value) : [...selected, opt.value],
                  )
                }}
                disabled={disabled}
                className={cn(
                  'px-3 py-1.5 rounded-full text-sm border transition-colors',
                  isActive
                    ? 'bg-primary text-primary-foreground border-primary'
                    : 'bg-card hover:bg-accent/50 border-border',
                )}
              >
                {opt.label}
              </button>
            )
          })}
        </div>
      </div>
    )
  }

  if (kind === 'yes_no') {
    return (
      <div>
        <p className="text-sm font-medium mb-2">{text}</p>
        <div className="flex gap-2">
          <button
            type="button"
            onClick={() => onChange('yes')}
            disabled={disabled}
            className={cn(
              'px-4 py-1.5 rounded-lg text-sm border transition-colors',
              value === 'yes'
                ? 'bg-success/15 text-success border-success/40'
                : 'bg-card hover:bg-accent/50 border-border',
            )}
          >
            ✅ {t('chat.interview.yes')}
          </button>
          <button
            type="button"
            onClick={() => onChange('no')}
            disabled={disabled}
            className={cn(
              'px-4 py-1.5 rounded-lg text-sm border transition-colors',
              value === 'no'
                ? 'bg-error/15 text-error border-error/40'
                : 'bg-card hover:bg-accent/50 border-border',
            )}
          >
            ❌ {t('chat.interview.no')}
          </button>
        </div>
      </div>
    )
  }

  // free_text (default)
  return (
    <div>
      <p className="text-sm font-medium mb-2">{text}</p>
      <textarea
        value={(value as string) ?? ''}
        onChange={(e) => onChange(e.target.value)}
        disabled={disabled}
        className="flex min-h-[44px] w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50 resize-none"
        placeholder={t('chat.interview.freeTextPlaceholder')}
      />
    </div>
  )
}
