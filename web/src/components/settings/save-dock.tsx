import { AlertTriangle, Check, Eye, X } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'

interface SaveDockProps {
  /** Total number of changed fields. */
  totalChanges: number
  /** Number of changes that require a restart. */
  restartRequired: number
  /** Number of changes that apply live. */
  applyLive: number
  /** Whether the save mutation is in flight. */
  isPending?: boolean
  /** Open the diff preview modal. */
  onReview: () => void
  /** Discard all changes and refetch from server. */
  onDiscard: () => void
  /** Whether the dock should be visible. */
  visible: boolean
}

/**
 * Floating "save dock" pinned to the bottom-right of the settings page.
 *
 * The dock is purely a presentation layer — it only renders the
 * aggregate state and forwards user actions. The host page owns the
 * form state, the diff, and the save mutation.
 */
export function SaveDock({
  totalChanges,
  restartRequired,
  applyLive,
  isPending,
  onReview,
  onDiscard,
  visible,
}: SaveDockProps) {
  const { t } = useTranslation()

  if (!visible || totalChanges === 0) return null

  return (
    <section
      aria-label={t('settings.saveDockLabel')}
      data-testid="save-dock"
      className={cn(
        'fixed bottom-5 right-5 z-50',
        // entrance / exit animation
        'animate-fade-in-up',
      )}
    >
      <div
        className={cn(
          'flex items-center gap-3 rounded-xl border bg-card/90 px-4 py-3 shadow-xl backdrop-blur',
          'dark:border-white/[0.08]',
        )}
      >
        {/* Counts */}
        <div className="flex items-center gap-3 pr-2">
          <div className="flex items-center gap-1.5">
            <span className="inline-block h-2 w-2 rounded-full bg-primary" />
            <span className="text-sm font-medium tabular-nums">{totalChanges}</span>
            <span className="text-xs text-muted-foreground">
              {totalChanges === 1 ? t('settings.change_one') : t('settings.change_other')}
            </span>
          </div>
          {restartRequired > 0 && (
            <div className="flex items-center gap-1 text-warning">
              <AlertTriangle className="h-3.5 w-3.5" />
              <span className="text-xs font-medium tabular-nums">{restartRequired}</span>
            </div>
          )}
          {applyLive > 0 && (
            <div className="hidden sm:flex items-center gap-1 text-success">
              <Check className="h-3.5 w-3.5" />
              <span className="text-xs font-medium tabular-nums">{applyLive}</span>
            </div>
          )}
        </div>

        <span aria-hidden className="h-6 w-px bg-border" />

        {/* Actions */}
        <div className="flex items-center gap-2">
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={onDiscard}
            disabled={isPending}
            className="text-muted-foreground hover:text-foreground"
            data-testid="save-dock-discard"
          >
            <X className="h-3.5 w-3.5 mr-1.5" />
            {t('settings.discardChanges')}
          </Button>
          <Button
            type="button"
            size="sm"
            onClick={onReview}
            disabled={isPending}
            data-testid="save-dock-review"
          >
            <Eye className="h-3.5 w-3.5 mr-1.5" />
            {t('settings.reviewChanges')}
          </Button>
        </div>
      </div>
    </section>
  )
}
