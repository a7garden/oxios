import { useTranslation } from 'react-i18next'
import { cn } from '@/lib/utils'

type Filter = 'all' | 'ready' | 'needs_setup' | 'disabled'

interface SkillSummaryPillProps {
  counts: Record<string, number>
  filter: Filter
  onFilterChange: (f: Filter) => void
}

/**
 * Status summary that REPLACES the manual refresh button (design F5).
 * Each segment both reports a count and acts as a filter trigger.
 */
export function SkillSummaryPill({ counts, filter, onFilterChange }: SkillSummaryPillProps) {
  const { t } = useTranslation()
  const segs: { key: Filter; label: string; count: number; dot?: string }[] = [
    { key: 'all', label: t('common.all'), count: counts.all ?? 0 },
    { key: 'ready', label: t('skills.summaryReady'), count: counts.ready ?? 0, dot: 'bg-success' },
    {
      key: 'needs_setup',
      label: t('skills.summaryNeedsSetup'),
      count: counts.needs_setup ?? 0,
      dot: 'bg-warning',
    },
    {
      key: 'disabled',
      label: t('skills.summaryDisabled'),
      count: counts.disabled ?? 0,
      dot: 'bg-muted-foreground/50',
    },
  ]

  return (
    <div className="inline-flex h-9 items-center rounded-lg bg-muted p-1 text-muted-foreground gap-0.5">
      {segs.map((s) => (
        <button
          key={s.key}
          type="button"
          onClick={() => onFilterChange(s.key)}
          className={cn(
            'inline-flex items-center gap-1.5 whitespace-nowrap rounded-md px-3 py-1 text-xs font-medium transition-all',
            filter === s.key ? 'bg-background text-foreground shadow' : 'hover:bg-background/50',
          )}
        >
          {s.dot && <span className={cn('h-1.5 w-1.5 rounded-full', s.dot)} />}
          <span className="font-mono">{s.count}</span>
          <span className="hidden sm:inline">{s.label}</span>
        </button>
      ))}
    </div>
  )
}
