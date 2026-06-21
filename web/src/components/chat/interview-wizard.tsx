import { ArrowLeft, ArrowRight, ClipboardList, HelpCircle, SkipForward } from 'lucide-react'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { Textarea } from '@/components/ui/textarea'
import { cn } from '@/lib/utils'
import type { InterviewAnswer, InterviewQuestion } from '@/types'

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

interface InterviewWizardProps {
  questions: InterviewQuestion[]
  round: number
  ambiguity: number
  onSubmit: (answers: InterviewAnswer[]) => void
  disabled?: boolean
}

// ---------------------------------------------------------------------------
// Main Wizard Component
// ---------------------------------------------------------------------------

export function InterviewWizard({
  questions,
  round,
  ambiguity,
  onSubmit,
  disabled,
}: InterviewWizardProps) {
  const { t } = useTranslation()
  const [currentStep, setCurrentStep] = useState(0)
  const [answers, setAnswers] = useState<Record<string, string | string[]>>({})
  const [freeTexts, setFreeTexts] = useState<Record<string, string>>({})
  const [, setDirection] = useState<'forward' | 'back'>('forward')
  const freeTextRef = useRef<HTMLTextAreaElement>(null)
  const contentRef = useRef<HTMLDivElement>(null)

  const totalSteps = questions.length
  const question = questions[currentStep]
  const isLastStep = currentStep === totalSteps - 1

  // Derived clarity (1 - ambiguity), clamped
  const clarity = Math.max(0, Math.min(1, 1 - ambiguity))
  const clarityPercent = Math.round(clarity * 100)

  // Clarity bar color
  const barColor = clarity > 0.7 ? 'bg-success' : clarity > 0.4 ? 'bg-warning' : 'bg-error'

  const setAnswer = (qid: string, value: string | string[]) => {
    setAnswers((prev) => ({ ...prev, [qid]: value }))
  }

  const toggleMulti = (qid: string, value: string) => {
    setAnswers((prev) => {
      const current = (prev[qid] as string[]) ?? []
      const next = current.includes(value)
        ? current.filter((v) => v !== value)
        : [...current, value]
      return { ...prev, [qid]: next }
    })
  }

  const handleNext = useCallback(() => {
    if (isLastStep) {
      // Submit all answers
      const formatted: InterviewAnswer[] = []

      for (const q of questions) {
        const structured = answers[q.id]
        const free = freeTexts[q.id]?.trim()

        // Build the value: merge structured + free text
        const parts: string[] = []
        if (structured) {
          if (Array.isArray(structured)) {
            parts.push(...structured.filter(Boolean))
          } else if (structured.trim()) {
            parts.push(structured)
          }
        }
        if (free) {
          parts.push(free)
        }

        if (parts.length > 0) {
          formatted.push({
            question_id: q.id,
            value: parts.join('; '),
          })
        }
      }

      onSubmit(formatted)
    } else {
      setDirection('forward')
      setCurrentStep((s) => s + 1)
    }
  }, [isLastStep, questions, answers, freeTexts, onSubmit])

  const handlePrev = useCallback(() => {
    if (currentStep > 0) {
      setDirection('back')
      setCurrentStep((s) => s - 1)
    }
  }, [currentStep])

  const handleSkip = useCallback(() => {
    if (isLastStep) {
      // Submit whatever we have (skipped questions have no answer)
      const formatted: InterviewAnswer[] = []
      for (const q of questions) {
        const structured = answers[q.id]
        const free = freeTexts[q.id]?.trim()
        const parts: string[] = []
        if (structured) {
          if (Array.isArray(structured)) {
            parts.push(...structured.filter(Boolean))
          } else if (structured.trim()) {
            parts.push(structured)
          }
        }
        if (free) parts.push(free)
        if (parts.length > 0) {
          formatted.push({ question_id: q.id, value: parts.join('; ') })
        }
      }
      onSubmit(formatted)
    } else {
      setDirection('forward')
      setCurrentStep((s) => s + 1)
    }
  }, [isLastStep, questions, answers, freeTexts, onSubmit])

  // Move focus between options with arrow keys (roving focus). Returns the
  // value of the newly-focused option (via `data-value`) so single-choice
  // questions can select on arrow — matching native radiogroup behavior.
  // Returns undefined when there are no option buttons (e.g. free_text).
  const focusOption = useCallback((dir: 1 | -1): string | undefined => {
    const container = contentRef.current
    if (!container) return undefined
    const btns = Array.from(
      container.querySelectorAll<HTMLButtonElement>('button[data-option="true"]'),
    )
    if (btns.length === 0) return undefined
    const active = document.activeElement as HTMLButtonElement | null
    const idx = active ? btns.indexOf(active) : -1
    const next = idx === -1 ? 0 : Math.min(Math.max(idx + dir, 0), btns.length - 1)
    const target = btns[next]
    target?.focus()
    return target?.dataset.value
  }, [])

  // Autofocus on step change so the whole wizard is keyboard-operable without
  // a mouse: choice questions focus the first option (Space toggles, arrows
  // roam, numbers pick), free-text questions focus the textarea.
  useEffect(() => {
    const q = questions[currentStep]
    if (!q) return
    const id = requestAnimationFrame(() => {
      if (q.kind === 'free_text') {
        freeTextRef.current?.focus()
      } else {
        contentRef.current?.querySelector<HTMLButtonElement>('button[data-option="true"]')?.focus()
      }
    })
    return () => cancelAnimationFrame(id)
  }, [currentStep, questions])

  // Global keyboard shortcuts. Enter advances from ANYWHERE — including the
  // free-text textarea (Shift+Enter inserts a newline). IME composition Enter
  // is ignored so Korean/CJK users can confirm candidates without advancing.
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Respect the disabled state (e.g. mid-stream) just like the footer
      // buttons do — don't let a stray Enter submit while the wizard is locked.
      if (disabled) return

      const target = e.target as HTMLElement | null
      const tag = target?.tagName
      const inText = tag === 'TEXTAREA' || tag === 'INPUT'

      // Enter advances from the wizard's own textarea OR from any non-text
      // target (option buttons, the card body). preventDefault also cancels
      // native <button> activation, so Enter on a focused option advances
      // instead of toggling it. Other inputs on the page are left alone.
      // Shift+Enter inserts a newline; IME-composition Enter confirms a
      // candidate without advancing (Korean/CJK).
      const composing = e.isComposing || e.keyCode === 229
      const isWizardTextarea = target === freeTextRef.current
      if (e.key === 'Enter' && !e.shiftKey && !composing && (!inText || isWizardTextarea)) {
        e.preventDefault()
        handleNext()
        return
      }

      // Inside some other text field — don't hijack it.
      if (inText) return

      if (e.key === 'Backspace') {
        e.preventDefault()
        handlePrev()
      } else if (e.key === 'Escape') {
        e.preventDefault()
        handleSkip()
      } else if (e.key === 'ArrowDown' || e.key === 'ArrowRight') {
        e.preventDefault()
        const v = focusOption(1)
        // Native radiogroup semantics: arrow both moves AND selects for
        // single-choice. Multi-choice only moves focus (Space toggles).
        if (question?.kind === 'single_choice' && v) setAnswer(question.id, v)
      } else if (e.key === 'ArrowUp' || e.key === 'ArrowLeft') {
        e.preventDefault()
        const v = focusOption(-1)
        if (question?.kind === 'single_choice' && v) setAnswer(question.id, v)
      } else if (question && question.kind !== 'free_text') {
        // Number keys 1-9 for quick selection (no focus needed)
        const num = parseInt(e.key, 10)
        if (num >= 1 && num <= 9) {
          const opts = question.options ?? []
          if (num <= opts.length) {
            e.preventDefault()
            const opt = opts[num - 1]!
            if (question.kind === 'multi_choice') {
              toggleMulti(question.id, opt.value)
            } else {
              setAnswer(question.id, opt.value)
            }
          }
        }
      }
    }

    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [disabled, question, handleNext, handlePrev, handleSkip, focusOption])

  // Current free text for this step
  const currentFreeText = freeTexts[question?.id ?? ''] ?? ''

  // Answered dots for progress
  const answeredDots = useMemo(
    () =>
      questions.map((q) => {
        const v = answers[q.id]
        if (v === undefined) return false
        if (typeof v === 'string') return v.trim().length > 0
        return (v as string[]).length > 0
      }),
    [questions, answers],
  )

  if (!question) return null

  return (
    <div className="flex gap-3 my-1.5">
      <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-primary text-primary-foreground">
        <HelpCircle className="h-4 w-4" />
      </div>
      <div className="max-w-[85%] flex-1 min-w-0">
        <div className="rounded-xl border bg-card shadow-sm overflow-hidden">
          {/* ── Header ── */}
          <div className="flex items-center justify-between px-4 py-2.5 border-b bg-muted/30">
            <div className="flex items-center gap-2">
              <ClipboardList className="h-3.5 w-3.5 text-muted-foreground" />
              <span className="text-sm font-medium">{t('chat.interview.wizardTitle')}</span>
              <span className="text-xs text-muted-foreground ml-1">
                {t('chat.interview.roundLabel', { round })}
              </span>
            </div>
            <div className="flex items-center gap-2">
              <span className="text-xs text-muted-foreground">
                {t('chat.interview.clarity', { percent: clarityPercent })}
              </span>
              <div className="w-16 h-1.5 rounded-full bg-muted overflow-hidden">
                <div
                  className={`h-full rounded-full transition-all duration-500 ${barColor}`}
                  style={{ width: `${clarityPercent}%` }}
                />
              </div>
            </div>
          </div>

          {/* ── Progress dots ── */}
          <div className="flex items-center justify-center gap-1.5 pt-3 pb-1">
            {questions.map((_, i) => (
              <button
                key={i}
                type="button"
                onClick={() => {
                  setDirection(i > currentStep ? 'forward' : 'back')
                  setCurrentStep(i)
                }}
                className={cn(
                  'w-2 h-2 rounded-full transition-all',
                  i === currentStep
                    ? 'bg-primary scale-125'
                    : answeredDots[i]
                      ? 'bg-primary/50'
                      : 'bg-muted-foreground/25',
                )}
                aria-label={`Question ${i + 1}`}
              />
            ))}
          </div>

          {/* ── Step counter ── */}
          <div className="text-center text-xs text-muted-foreground pb-1">
            {t('chat.interview.stepOf', {
              current: currentStep + 1,
              total: totalSteps,
            })}
          </div>

          {/* ── Question content (animated) ── */}
          <div ref={contentRef} className="px-4 pt-2 pb-3">
            <div
              key={currentStep}
              className="animate-in fade-in-0 slide-in-from-bottom-2 duration-200"
            >
              {/* Question text */}
              <p className="text-sm font-medium mb-3 leading-relaxed">{question.text}</p>

              {/* Structured widget */}
              <QuestionWidget
                question={question}
                value={answers[question.id]}
                onChange={(v) => setAnswer(question.id, v)}
                onToggle={(v) => toggleMulti(question.id, v)}
                disabled={disabled}
              />

              {/* Always-visible free text */}
              <div className="mt-3">
                <Textarea
                  ref={freeTextRef}
                  value={currentFreeText}
                  onChange={(e) =>
                    setFreeTexts((prev) => ({
                      ...prev,
                      [question.id]: e.target.value,
                    }))
                  }
                  placeholder={t('chat.interview.orType')}
                  className="min-h-[44px] resize-none text-sm bg-muted/30"
                  disabled={disabled}
                />
              </div>
            </div>
          </div>

          {/* ── Footer ── */}
          <div className="flex items-center justify-between px-4 py-2.5 border-t bg-muted/20">
            <div className="flex items-center gap-1">
              {currentStep > 0 && (
                <Button
                  onClick={handlePrev}
                  variant="ghost"
                  size="icon"
                  disabled={disabled}
                  className="h-7 w-7"
                  title={t('chat.interview.previous')}
                >
                  <ArrowLeft className="h-3.5 w-3.5" />
                </Button>
              )}
              <Button
                onClick={handleSkip}
                variant="ghost"
                size="icon"
                disabled={disabled}
                className="h-7 w-7 text-muted-foreground"
                title={t('chat.interview.skip')}
              >
                <SkipForward className="h-3.5 w-3.5" />
              </Button>
            </div>
            <Button
              onClick={handleNext}
              disabled={disabled}
              size="icon"
              className="h-7 w-7"
              title={isLastStep ? t('chat.interview.submit') : t('chat.interview.next')}
            >
              <ArrowRight className="h-3.5 w-3.5" />
            </Button>
          </div>

          {/* ── Keyboard hint ── */}
          <div className="px-4 py-1.5 border-t text-center">
            <p className="text-[10px] text-muted-foreground/60">
              {t('chat.interview.keyboardHint')}
            </p>
          </div>
        </div>
      </div>
    </div>
  )
}

// ---------------------------------------------------------------------------
// Question Widget (per-kind rendering)
// ---------------------------------------------------------------------------

function QuestionWidget({
  question,
  value,
  onChange,
  onToggle,
  disabled,
}: {
  question: InterviewQuestion
  value: string | string[] | undefined
  onChange: (values: string | string[]) => void
  onToggle: (value: string) => void
  disabled?: boolean
}) {
  const { t } = useTranslation()

  if (question.kind === 'yes_no') {
    return (
      // biome-ignore lint/a11y/useSemanticElements: custom-styled choice widget; ARIA group is intentional for keyboard/styling integration
      <div className="flex gap-2" role="group" aria-label={question.text}>
        <button
          type="button"
          onClick={() => onChange('yes')}
          data-option="true"
          data-value="yes"
          aria-pressed={value === 'yes'}
          disabled={disabled}
          className={cn(
            'flex-1 flex items-center justify-center gap-2 px-4 py-3 rounded-lg text-sm border transition-all',
            value === 'yes'
              ? 'bg-success/15 text-success border-success/40 shadow-sm'
              : 'bg-card hover:bg-accent/50 border-border',
          )}
        >
          ✅ {t('chat.interview.yes')}
        </button>
        <button
          type="button"
          onClick={() => onChange('no')}
          data-option="true"
          data-value="no"
          aria-pressed={value === 'no'}
          disabled={disabled}
          className={cn(
            'flex-1 flex items-center justify-center gap-2 px-4 py-3 rounded-lg text-sm border transition-all',
            value === 'no'
              ? 'bg-error/15 text-error border-error/40 shadow-sm'
              : 'bg-card hover:bg-accent/50 border-border',
          )}
        >
          ❌ {t('chat.interview.no')}
        </button>
      </div>
    )
  }

  if (question.kind === 'multi_choice') {
    const selected = Array.isArray(value) ? value : []
    return (
      <div>
        {selected.length > 0 && (
          <p className="text-xs text-muted-foreground mb-2">
            {t('chat.interview.selected', { count: selected.length })}
          </p>
        )}
        {/* biome-ignore lint/a11y/useSemanticElements: custom-styled choice widget; ARIA group is intentional for keyboard/styling integration */}
        <div className="flex flex-wrap gap-2" role="group" aria-label={question.text}>
          {(question.options ?? []).map((opt, i) => {
            const isActive = selected.includes(opt.value)
            return (
              <button
                key={opt.value}
                type="button"
                onClick={() => onToggle(opt.value)}
                data-option="true"
                data-value={opt.value}
                aria-pressed={isActive}
                disabled={disabled}
                className={cn(
                  'px-3 py-2 rounded-lg text-sm border transition-all text-left',
                  isActive
                    ? 'bg-primary text-primary-foreground border-primary shadow-sm'
                    : 'bg-card hover:bg-accent/50 border-border',
                )}
              >
                <span className="flex items-center gap-1.5">
                  <span className="text-xs opacity-60">{i + 1}.</span>
                  {isActive ? '☑' : '☐'} {opt.label}
                </span>
                {opt.description && isActive && (
                  <span className="block text-xs opacity-70 mt-1 ml-5">{opt.description}</span>
                )}
              </button>
            )
          })}
        </div>
      </div>
    )
  }

  if (question.kind === 'single_choice') {
    return (
      <div className="grid gap-2 sm:grid-cols-2" role="radiogroup" aria-label={question.text}>
        {(question.options ?? []).map((opt, i) => (
          // biome-ignore lint/a11y/useSemanticElements: button-styled radio — custom keyboard (arrows select) and chip styling need a button, not a native input
          <button
            key={opt.value}
            type="button"
            role="radio"
            aria-checked={value === opt.value}
            onClick={() => onChange(opt.value)}
            data-option="true"
            data-value={opt.value}
            disabled={disabled}
            className={cn(
              'px-3 py-2.5 rounded-lg text-sm border transition-all text-left',
              value === opt.value
                ? 'bg-primary text-primary-foreground border-primary shadow-sm'
                : 'bg-card hover:bg-accent/50 border-border',
            )}
          >
            <span className="flex items-center gap-1.5">
              <span className="text-xs opacity-60">{i + 1}.</span>
              {opt.label}
            </span>
            {opt.description && value === opt.value && (
              <span className="block text-xs opacity-70 mt-1 ml-5">{opt.description}</span>
            )}
          </button>
        ))}
      </div>
    )
  }

  // free_text — no structured widget, just the always-visible textarea
  return null
}
