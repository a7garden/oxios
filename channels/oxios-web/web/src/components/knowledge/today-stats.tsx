import { useKnowledgeStatsToday, useKnowledgeDoneToday } from '@/hooks/use-knowledge'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Activity } from 'lucide-react'

export function TodayStats() {
  const { data: report, isLoading: reportLoading } = useKnowledgeStatsToday()
  const { data: doneData, isLoading: doneLoading } = useKnowledgeDoneToday()

  if (reportLoading || doneLoading) {
    return <div className="p-4 text-sm text-muted-foreground">Loading stats...</div>
  }

  const doneItems = doneData?.items ?? []
  const doneCount = doneData?.count ?? 0

  return (
    <Card>
      <CardHeader className="pb-2">
        <CardTitle className="text-sm flex items-center gap-2">
          <Activity className="h-4 w-4" />
          Today
        </CardTitle>
      </CardHeader>
      <CardContent>
        {doneCount > 0 ? (
          <div className="space-y-1">
            <p className="text-2xl font-bold">{doneCount}</p>
            <p className="text-xs text-muted-foreground">items completed</p>
            {doneItems.length > 0 && (
              <ul className="mt-2 space-y-0.5">
                {doneItems.slice(0, 5).map((item: unknown, i: number) => (
                  <li key={i} className="text-xs text-muted-foreground truncate">
                    • {String(item)}
                  </li>
                ))}
              </ul>
            )}
          </div>
        ) : (
          <p className="text-sm text-muted-foreground">Nothing completed yet today</p>
        )}
        {report && Object.keys(report).length > 0 && (
          <details className="mt-3">
            <summary className="text-xs text-muted-foreground cursor-pointer">Raw report</summary>
            <pre className="text-xs bg-muted p-2 rounded mt-1 overflow-x-auto">
              {JSON.stringify(report, null, 2)}
            </pre>
          </details>
        )}
      </CardContent>
    </Card>
  )
}