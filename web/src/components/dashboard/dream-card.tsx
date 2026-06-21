import { Moon, Sunrise } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { useDreamStatus } from '@/hooks/use-memory'
import { formatRelativeDate } from '@/lib/utils'

/**
 * Dream (memory consolidation) status card for the dashboard.
 *
 * Shows whether Dream is currently running, last/next run time,
 * and cycles completed. Always renders — shows a placeholder
 * when Dream data is unavailable to keep the grid stable.
 */
export function DreamCard({ className }: { className?: string }) {
  const { t } = useTranslation()
  const { data: dream, isLoading } = useDreamStatus()

  const isRunning = dream?.running ?? false
  const lastRun = dream?.last_run ? formatRelativeDate(dream.last_run, t) : '—'
  const nextRun = dream?.next_run ? formatRelativeDate(dream.next_run, t) : '—'

  return (
    <Card className={className}>
      <CardHeader className="flex flex-row items-center justify-between pb-2">
        <CardTitle className="flex items-center gap-2 text-base">
          {isRunning ? (
            <Sunrise className="h-4 w-4 text-info animate-pulse" />
          ) : (
            <Moon className="h-4 w-4" />
          )}
          {t('dashboard.dream')}
        </CardTitle>
        {isRunning && (
          <span className="text-2xs font-medium text-info animate-pulse">
            {t('dashboard.running')}
          </span>
        )}
      </CardHeader>
      <CardContent className="pt-0">
        {isLoading ? (
          <p className="text-xs text-muted-foreground py-1">{t('common.loading')}</p>
        ) : !dream ? (
          <p className="text-xs text-muted-foreground py-1">{t('dashboard.dreamUnavailable')}</p>
        ) : (
          <div className="space-y-1.5">
            <div className="flex items-center justify-between text-xs">
              <span className="text-muted-foreground">{t('dashboard.lastRun')}</span>
              <span>{lastRun}</span>
            </div>
            <div className="flex items-center justify-between text-xs">
              <span className="text-muted-foreground">{t('dashboard.nextRun')}</span>
              <span>{nextRun}</span>
            </div>
            <div className="flex items-center justify-between text-xs">
              <span className="text-muted-foreground">{t('dashboard.cyclesCompleted')}</span>
              <span className="font-semibold">{dream.cycles_completed}</span>
            </div>
          </div>
        )}
      </CardContent>
    </Card>
  )
}
