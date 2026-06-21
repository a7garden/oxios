import { HelpCircle } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { Textarea } from '@/components/ui/textarea'
import type { InterviewAnswer, InterviewQuestion } from '@/types'
import { InterviewQuestionCard } from './interview-question-card'

interface InterviewResponseProps {
  questions: InterviewQuestion[]
  round: number
  ambiguity: number
  onSubmit: (answers: InterviewAnswer[]) => void
  disabled?: boolean
}

/**
 * Interactive interview UI. Renders structured questions as widgets
 * (chips, yes/no buttons, text inputs) and collects answers.
 * Submitted answers are forwarded as a natural language message.
 */
export function InterviewResponse({
  questions,
  round,
  ambiguity,
  onSubmit,
  disabled,
}: InterviewResponseProps) {
  const { t } = useTranslation()
  const [answers, setAnswers] = useState<Record<string, string | string[]>>({})
  const [freeText, setFreeText] = useState('')

  const handleSubmit = () => {
    const formatted: InterviewAnswer[] = Object.entries(answers)
      .map(([qId, value]) => ({
        question_id: qId,
        value: Array.isArray(value) ? value.join(', ') : value,
      }))
      .filter((a) => a.value.trim())
    if (freeText.trim()) {
      formatted.push({ question_id: 'free_text', value: freeText.trim() })
    }
    onSubmit(formatted)
  }

  const allChoiceAnswered = questions
    .filter((q) => q.kind !== 'free_text')
    .every((q) => {
      const v = answers[q.id]
      return v !== undefined && (typeof v === 'string' ? v.trim() : (v as string[]).length > 0)
    })

  // Ambiguity bar: 1.0 = fully ambiguous (red), 0.0 = clear (green)
  const clarity = Math.max(0, Math.min(1, 1 - ambiguity))
  const barColor = clarity > 0.7 ? 'bg-success' : clarity > 0.4 ? 'bg-warning' : 'bg-error'

  return (
    <div className="flex gap-3 my-1.5">
      <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-primary text-primary-foreground">
        <HelpCircle className="h-4 w-4" />
      </div>
      <div className="max-w-[80%] flex-1">
        <div className="rounded-xl border bg-card shadow-sm">
          {/* Header */}
          <div className="flex items-center justify-between px-4 py-3 border-b">
            <span className="text-sm font-medium">{t('chat.interview.title')}</span>
            <div className="flex items-center gap-2 text-xs text-muted-foreground">
              <span>{t('chat.interview.roundLabel', { round })}</span>
              <div className="w-16 h-1.5 rounded-full bg-muted overflow-hidden">
                <div
                  className={`h-full rounded-full transition-all ${barColor}`}
                  style={{ width: `${clarity * 100}%` }}
                />
              </div>
            </div>
          </div>

          {/* Questions */}
          <div className="p-4 space-y-5">
            {questions.map((q) => (
              <InterviewQuestionCard
                key={q.id}
                question={q}
                value={answers[q.id]}
                onChange={(v) => setAnswers((prev) => ({ ...prev, [q.id]: v }))}
                disabled={disabled}
              />
            ))}

            {/* Free-text area for extra context */}
            <div>
              <p className="text-xs text-muted-foreground mb-1.5">
                {t('chat.interview.additionalThoughts')}
              </p>
              <Textarea
                value={freeText}
                onChange={(e) => setFreeText(e.target.value)}
                placeholder={t('chat.interview.optionalPlaceholder')}
                className="min-h-[60px] resize-none text-sm"
                disabled={disabled}
              />
            </div>
          </div>

          {/* Submit */}
          <div className="flex justify-end px-4 py-3 border-t">
            <Button onClick={handleSubmit} disabled={!allChoiceAnswered || disabled} size="sm">
              {t('chat.interview.submit')}
            </Button>
          </div>
        </div>
      </div>
    </div>
  )
}
