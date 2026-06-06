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
  /** If true, only render the input row (no tags yet). */
}

/**
 * Multi-line tag input for `exec.allowed_commands`. Enter or comma
 * adds a new tag; backspace on empty input removes the last tag.
 */
export function ExecAllowlistEditor({
  value,
  onChange,
  disabled,
  className,
}: ExecAllowlistEditorProps) {
  const { t } = useTranslation()
  const [draft, setDraft] = useState('')

  const commit = (raw: string) => {
    const next = raw
      .split(/[\s,]+/)
      .map((s) => s.trim())
      .filter((s) => s.length > 0)
    if (next.length === 0) return
    // De-duplicate.
    const merged = Array.from(new Set([...value, ...next]))
    onChange(merged)
    setDraft('')
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
    }
  }

  return (
    <div className={cn('space-y-2', className)}>
      <div className="flex flex-wrap gap-1.5 rounded-md border bg-muted/30 p-2 min-h-[2.5rem]">
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
        <Input
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={handleKeyDown}
          onBlur={() => commit(draft)}
          placeholder={value.length === 0 ? t('settings.allowedCommandsPlaceholder') : ''}
          disabled={disabled}
          className="flex-1 min-w-[120px] h-7 border-0 bg-transparent shadow-none focus-visible:ring-0 px-1"
        />
      </div>
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
