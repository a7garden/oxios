import { AlertTriangle } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import type { EventConflict } from '@/types/calendar'

interface Props {
  conflicts: EventConflict[]
}

export function ConflictWarning({ conflicts }: Props) {
  const { t } = useTranslation()
  if (conflicts.length === 0) return null

  return (
    <div className="rounded-md bg-warning-subtle border border-warning-subtle-border p-3">
      <div className="flex items-center gap-2 text-warning text-sm font-medium">
        <AlertTriangle className="h-4 w-4" />
        {t('calendar.conflictTitle', { count: conflicts.length })}
      </div>
      <ul className="mt-1 text-xs text-warning/80 space-y-0.5">
        {conflicts.map((c) => (
          <li key={c.uid}>
            {c.title} ({t('calendar.conflictDetail', { minutes: c.overlap_minutes })})
          </li>
        ))}
      </ul>
    </div>
  )
}
