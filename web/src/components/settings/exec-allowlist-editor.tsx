import { X } from 'lucide-react'
import { type KeyboardEvent, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { cn } from '@/lib/utils'

interface ExecAllowlistEditorProps {
  value: string[]
  onChange: (next: string[]) => void
  disabled?: boolean
  className?: string
  /**
   * If provided, every committed value is validated before being added.
   * Returns an i18n key for the error message, or null if valid.
   * Used by tag inputs to validate a value before adding.
   */
  validate?: (value: string) => string | null
  /**
   * Optional list of suggested values with display labels and optional group.
   * When provided, the input shows a suggestion popover.
   * Used by `allowed_tools` to show the tool catalog.
   */
  suggestions?: { value: string; label: string; group?: string }[]
}

/**
 * Multi-line tag input for `exec.allowed_commands`, `allowed_tools`,
 * etc. Enter or comma adds a new tag; backspace on
 * empty input removes the last tag.
 *
 * - `suggestions` prop → suggestion popover (tool catalog)
 * - `validate` prop → inline error on invalid input
 */
export function ExecAllowlistEditor({
  value,
  onChange,
  disabled,
  className,
  validate,
  suggestions,
}: ExecAllowlistEditorProps) {
  const { t } = useTranslation()
  const [draft, setDraft] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [showSuggestions, setShowSuggestions] = useState(false)

  const filteredSuggestions = suggestions
    ? suggestions.filter(
        (s) => !value.includes(s.value) && s.value.toLowerCase().includes(draft.toLowerCase()),
      )
    : []

  const commit = (raw: string) => {
    const trimmed = raw.trim()
    if (!trimmed) return

    // Validate if prop provided.
    if (validate) {
      const err = validate(trimmed)
      if (err) {
        setError(err)
        return
      }
    }
    setError(null)

    // De-duplicate.
    if (value.includes(trimmed)) {
      setDraft('')
      return
    }
    onChange([...value, trimmed])
    setDraft('')
    setShowSuggestions(false)
  }

  const remove = (idx: number) => {
    onChange(value.filter((_, i) => i !== idx))
  }

  const handleKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter' || e.key === ',') {
      e.preventDefault()
      commit(draft)
    } else if (e.key === 'Backspace' && draft === '' && value.length > 0) {
      e.preventDefault()
      remove(value.length - 1)
    } else if (e.key === 'ArrowDown' && filteredSuggestions.length > 0) {
      e.preventDefault()
      setShowSuggestions(true)
    } else if (e.key === 'Escape') {
      setShowSuggestions(false)
    }
  }

  const handleSelectSuggestion = (suggestion: string) => {
    setDraft('')
    if (!value.includes(suggestion)) {
      onChange([...value, suggestion])
    }
    setShowSuggestions(false)
    setError(null)
  }

  // Group suggestions by category when available.
  const groupedSuggestions = suggestions
    ? filteredSuggestions.reduce<Record<string, typeof filteredSuggestions>>((acc, s) => {
        const group = s.group ?? t('common.other')
        if (!acc[group]) acc[group] = []
        acc[group].push(s)
        return acc
      }, {})
    : {}

  return (
    <div className={cn('space-y-2', className)}>
      <div
        className={cn(
          'flex flex-wrap gap-1.5 rounded-md border p-2 min-h-[2.5rem]',
          error ? 'border-destructive bg-destructive/5' : 'bg-muted/30',
        )}
      >
        {value.map((cmd, i) => (
          <span
            key={`${cmd}-${i}`}
            className="inline-flex items-center gap-1 rounded-md bg-background border px-2 py-1 text-xs font-mono"
          >
            {cmd}
            {!disabled && (
              <button
                type="button"
                aria-label={t('common.delete')}
                onClick={() => remove(i)}
                className="text-muted-foreground hover:text-foreground"
              >
                <X className="h-3 w-3" />
              </button>
            )}
          </span>
        ))}
        <div className="relative flex-1 min-w-[120px]">
          <Input
            value={draft}
            onChange={(e) => {
              setDraft(e.target.value)
              setError(null)
              if (suggestions && e.target.value) setShowSuggestions(true)
              else setShowSuggestions(false)
            }}
            onFocus={() => {
              if (suggestions && draft) setShowSuggestions(true)
            }}
            onBlur={() => {
              // Delay hiding so click on suggestion registers.
              setTimeout(() => setShowSuggestions(false), 200)
            }}
            onKeyDown={handleKeyDown}
            placeholder={
              value.length === 0 && !suggestions ? t('settings.allowedCommandsPlaceholder') : ''
            }
            disabled={disabled}
            className="h-7 border-0 bg-transparent shadow-none focus-visible:ring-0 px-1"
          />
          {/* Suggestions popover */}
          {showSuggestions && filteredSuggestions.length > 0 && (
            <div className="absolute left-0 top-full z-50 mt-1 w-64 rounded-md border bg-popover p-1 shadow-md">
              {Object.entries(groupedSuggestions).map(([group, items]) => (
                <div key={group}>
                  <div className="px-2 py-1 text-xs font-medium text-muted-foreground uppercase tracking-wider">
                    {t(`categories.${group}`, group)}
                  </div>
                  {items.map((s) => (
                    <button
                      key={s.value}
                      type="button"
                      className="flex w-full items-center gap-2 rounded-sm px-2 py-1.5 text-sm hover:bg-accent"
                      onMouseDown={(e) => {
                        e.preventDefault()
                        handleSelectSuggestion(s.value)
                      }}
                    >
                      <span className="font-mono text-xs">{s.value}</span>
                      <span className="text-xs text-muted-foreground ml-auto">{s.label}</span>
                    </button>
                  ))}
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
      {/* Inline error */}
      {error && <p className="text-xs text-destructive">{t(error)}</p>}
      {value.length > 0 && !disabled && (
        <Button
          type="button"
          variant="ghost"
          size="sm"
          onClick={() => onChange([])}
          className="text-xs text-muted-foreground"
        >
          {t('settings.clearAll')}
        </Button>
      )}
    </div>
  )
}
