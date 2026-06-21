import { Activity, Clock } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { EmptyState } from '@/components/shared/empty-state'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { useDreamReports, useDreamStatus } from '@/hooks/use-memory'
import type { DreamReport } from '@/types/memory'

export function DreamPanel() {
  const { t } = useTranslation()
  const { data: status, isLoading: sLoad } = useDreamStatus()
  const { data: reports, isLoading: rLoad, isError, refetch } = useDreamReports()

  if (isError) return <ErrorState onRetry={() => refetch()} />
  if (sLoad || rLoad) return <LoadingCards count={3} />

  const items = Array.isArray(reports) ? reports : []

  return (
    <div className="space-y-6">
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Activity className="h-5 w-5" /> {t('memory.dreamStatus')}
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="flex items-center gap-4 flex-wrap">
            <Badge variant={status?.running ? 'default' : 'secondary'}>
              {status?.running ? t('memory.dreamRunning') : t('memory.dreamIdle')}
            </Badge>
            {status?.last_run && (
              <span className="text-sm text-muted-foreground flex items-center gap-1">
                <Clock className="h-3 w-3" /> {t('memory.lastRun')}:{' '}
                {new Date(status.last_run).toLocaleString()}
              </span>
            )}
            {status?.cycles_completed != null && (
              <span className="text-sm text-muted-foreground">
                Cycles: {status.cycles_completed}
              </span>
            )}
          </div>
        </CardContent>
      </Card>
      {items.length === 0 ? (
        <EmptyState
          icon={<Activity className="h-10 w-10" />}
          title={t('memory.noDreamReports')}
          description={t('memory.noDreamReportsDescription')}
        />
      ) : (
        <div className="space-y-3">
          {items.map((r: DreamReport) => (
            <Card key={r.id}>
              <CardContent className="p-4">
                <div className="flex items-center justify-between mb-2">
                  <span className="text-sm font-medium">
                    {new Date(r.started_at).toLocaleString()}
                  </span>
                  <div className="flex items-center gap-2">
                    <Badge
                      variant={r.status === 'completed' ? 'success' : 'secondary'}
                      className="text-xs"
                    >
                      {r.status}
                    </Badge>
                    {r.completed_at && (
                      <span className="text-xs text-muted-foreground">
                        {(
                          (new Date(r.completed_at).getTime() - new Date(r.started_at).getTime()) /
                          1000
                        ).toFixed(1)}
                        s
                      </span>
                    )}
                  </div>
                </div>
                <div className="grid grid-cols-3 gap-2 text-xs">
                  <div>
                    <span className="text-muted-foreground">Processed:</span> {r.memories_processed}
                  </div>
                  <div>
                    <span className="text-muted-foreground">Consolidated:</span>{' '}
                    {r.memories_consolidated}
                  </div>
                  <div>
                    <span className="text-muted-foreground">Decayed:</span> {r.memories_decayed}
                  </div>
                </div>
                {r.summary && <p className="text-xs text-muted-foreground mt-2">{r.summary}</p>}
              </CardContent>
            </Card>
          ))}
        </div>
      )}
    </div>
  )
}
