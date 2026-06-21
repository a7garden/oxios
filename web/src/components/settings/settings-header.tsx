import { CircleDashed, CircleDot, Loader2 } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { cn } from '@/lib/utils'

type Status = 'saved' | 'unsaved' | 'saving' | 'error'

interface SettingsHeaderProps {
  title: string
  subtitle?: string
  status: Status
  /** Last save time (used for the "Saved at HH:MM" label). */
  lastSavedAt?: Date | null
  /** Total unsaved changes. */
  unsavedCount?: number
}

/**
 * Page header for `/settings`. Title, subtitle, and a small status pill
 * on the right that communicates the current save state.
 */
export function SettingsHeader({
  title,
  subtitle,
  status,
  lastSavedAt,
  unsavedCount = 0,
}: SettingsHeaderProps) {
  const { t } = useTranslation()
  return (
    <div className="flex items-end justify-between gap-4 pb-2">
      <div className="min-w-0">
        <h1 className="text-2xl font-bold tracking-tight text-foreground">{title}</h1>
        {subtitle && <p className="mt-1 text-sm text-muted-foreground">{subtitle}</p>}
      </div>
      <SaveStatusPill status={status} lastSavedAt={lastSavedAt} unsavedCount={unsavedCount} t={t} />
    </div>
  )
}

function SaveStatusPill({
  status,
  lastSavedAt,
  unsavedCount,
  t,
}: {
  status: Status
  lastSavedAt?: Date | null
  unsavedCount: number
  t: (k: string, opts?: Record<string, unknown>) => string
}) {
  if (status === 'saving') {
    return (
      <span
        className="inline-flex items-center gap-1.5 rounded-full border bg-muted/40 px-2.5 py-1 text-xs font-medium text-muted-foreground"
        data-testid="save-status-saving"
      >
        <Loader2 className="h-3 w-3 animate-spin" />
        {t('settings.saving')}
      </span>
    )
  }

  if (status === 'unsaved' || unsavedCount > 0) {
    return (
      <span
        className="inline-flex items-center gap-1.5 rounded-full border border-primary/30 bg-primary/5 px-2.5 py-1 text-xs font-medium text-primary"
        data-testid="save-status-unsaved"
      >
        <CircleDot className="h-3 w-3 fill-primary" />
        {t('settings.unsavedChanges')}
        <span className="tabular-nums">· {unsavedCount}</span>
      </span>
    )
  }

  if (status === 'error') {
    return (
      <span
        className="inline-flex items-center gap-1.5 rounded-full border border-error-subtle bg-error-subtle px-2.5 py-1 text-xs font-medium text-error"
        data-testid="save-status-error"
      >
        <CircleDashed className="h-3 w-3" />
        {t('settings.settingsSaveFailed')}
      </span>
    )
  }

  return (
    <span
      className={cn(
        'inline-flex items-center gap-1.5 rounded-full border bg-muted/30 px-2.5 py-1 text-xs font-medium text-muted-foreground',
      )}
      data-testid="save-status-saved"
    >
      <span className="inline-block h-1.5 w-1.5 rounded-full bg-success" />
      {lastSavedAt
        ? t('settings.savedAt', { time: formatTime(lastSavedAt) })
        : t('settings.settingsSaved')}
    </span>
  )
}

function formatTime(d: Date): string {
  return d.toLocaleTimeString(undefined, {
    hour: '2-digit',
    minute: '2-digit',
  })
}
