import { AlertTriangle } from 'lucide-react'
import type { EventConflict } from '@/types/calendar'

interface Props {
  conflicts: EventConflict[]
}

export function ConflictWarning({ conflicts }: Props) {
  if (conflicts.length === 0) return null

  return (
    <div className="rounded-md bg-amber-50 dark:bg-amber-900/20 border border-amber-200 dark:border-amber-800 p-3">
      <div className="flex items-center gap-2 text-amber-800 dark:text-amber-200 text-sm font-medium">
        <AlertTriangle className="h-4 w-4" />
        {conflicts.length}개 일정과 겹침
      </div>
      <ul className="mt-1 text-xs text-amber-700 dark:text-amber-300 space-y-0.5">
        {conflicts.map((c) => (
          <li key={c.uid}>
            {c.title} ({c.overlap_minutes}분 겹침)
          </li>
        ))}
      </ul>
    </div>
  )
}
