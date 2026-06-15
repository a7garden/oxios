import { RotateCcw } from 'lucide-react'
import type { ReactNode } from 'react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import { cn } from '@/lib/utils'

interface SectionCardProps {
  /** Section title (already translated). */
  title: string
  /** Section description (already translated). */
  description?: string
  /** Optional icon element. */
  icon?: ReactNode
  /** Section id for `data-section` attr. */
  sectionId: string
  /** Total field count. */
  fieldCount?: number
  /** Whether this section has any unsaved changes. */
  modified?: boolean
  /** Reset the section to server values. */
  onReset?: () => void
  /** Children — the field rows. */
  children: ReactNode
  /** Optional `className` passthrough for the outer Card. */
  className?: string
}

/**
 * Unified section container used by every settings section.
 *
 * Header carries the section title, an optional icon, a description,
 * a small metadata row (field count), and a "reset section" button
 * that only appears when there are unsaved changes.
 *
 * Restart-required information is **not** shown here — it only appears
 * in the DiffPreview modal at save time, so the resting state stays
 * clean and noise-free.
 *
 * The body is a clean stack of `FieldRow`s — separators are drawn by
 * each row's hover/active state, not by hard borders.
 */
export function SectionCard({
  title,
  description,
  icon,
  sectionId,
  fieldCount,
  modified,
  onReset,
  children,
  className,
}: SectionCardProps) {
  const { t } = useTranslation()

  return (
    <Card
      data-section={sectionId}
      data-modified={modified ? 'true' : undefined}
      className={cn(
        'overflow-hidden border transition-shadow',
        // Single-tone surface (spec §6): the whole card — header AND
        // body — sits on --surface-section (muted/30), giving a unified
        // inset-panel look instead of a white card with a tinted header.
        'bg-surface-section',
        'shadow-sm',
        modified && 'ring-1 ring-modified-accent/30 dark:ring-modified-accent/40',
        className,
      )}
    >
      {/* Header */}
      <div
        className={cn(
          'flex flex-col gap-3 border-b px-5 py-4 sm:flex-row sm:items-start sm:gap-4',
          // Single tone: no header gradient — same surface as the body.
        )}
      >
        {/* Title block */}
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2.5">
            {icon && (
              <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md bg-primary/10 text-primary">
                {icon}
              </span>
            )}
            <h2 className="text-base font-semibold tracking-tight text-foreground">{title}</h2>
            {modified && (
              <span
                aria-label={t('settings.modified')}
                title={t('settings.modified')}
                className="inline-block h-1.5 w-1.5 rounded-full bg-primary"
                data-testid="section-modified-dot"
              />
            )}
          </div>
          {description && (
            <p className="mt-1.5 text-sm text-muted-foreground leading-relaxed">{description}</p>
          )}

          {/* Metadata row */}
          {fieldCount !== undefined && fieldCount > 0 && (
            <div className="mt-3 flex flex-wrap items-center gap-2 text-2xs">
              <span className="text-muted-foreground tabular-nums">
                {fieldCount} {fieldCount === 1 ? 'field' : 'fields'}
              </span>
            </div>
          )}
        </div>

        {/* Reset button (only when modified) */}
        {onReset && (
          <div className="flex items-center gap-2 shrink-0">
            <Button
              type="button"
              variant="ghost"
              size="sm"
              onClick={onReset}
              disabled={!modified}
              className="text-muted-foreground hover:text-foreground"
              data-testid="section-reset"
            >
              <RotateCcw className="h-3.5 w-3.5 mr-1.5" />
              {t('common.reset')}
            </Button>
          </div>
        )}
      </div>

      {/* Body */}
      <CardContent className="px-5 py-4">
        <div className="divide-y divide-border/40">{children}</div>
      </CardContent>
    </Card>
  )
}
