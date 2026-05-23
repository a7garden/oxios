import { useQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { Calendar, CheckCircle, Clock, Loader2, RefreshCw } from 'lucide-react'
import { EmptyState } from '@/components/shared/empty-state'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { StatusIndicator } from '@/components/shared/status-indicator'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { api } from '@/lib/api-client'

interface SchedulerStatus {
  running: boolean
  total_tasks: number
  active_tasks: number
  max_concurrent: number
  tasks: SchedulerTask[]
}

interface SchedulerTask {
  id: string
  agent_id?: string
  priority: number
  status: 'queued' | 'running' | 'completed' | 'failed'
  created_at: string
  started_at?: string
}

export const Route = createFileRoute('/scheduler')({ component: SchedulerPage })

function SchedulerPage() {
  const { data, isLoading, isError, refetch, isFetching } = useQuery({
    queryKey: ['scheduler'],
    queryFn: async () => {
      // Backend has separate /api/scheduler/stats + /api/scheduler/tasks
      const [stats, tasksRes] = await Promise.all([
        api.get<{ queued: number; running: number; max_concurrent: number; rate_limit_per_minute: number; rate_remaining: number }>('/api/scheduler/stats'),
        api.get<{ queued: SchedulerTask[]; running: SchedulerTask[] }>('/api/scheduler/tasks'),
      ])
      return {
        running: stats.running > 0,
        total_tasks: stats.queued,
        active_tasks: stats.running,
        max_concurrent: stats.max_concurrent,
        tasks: [...(tasksRes.queued ?? []), ...(tasksRes.running ?? [])],
      } as SchedulerStatus
    },
    refetchInterval: 5000,
  })

  if (isLoading) return <LoadingCards count={4} />
  if (isError) return <ErrorState onRetry={() => refetch()} />

  const tasks = data?.tasks ?? []

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Scheduler</h1>
          <p className="text-muted-foreground">Task scheduling and queue management</p>
        </div>
        <button
          type="button"
          onClick={() => refetch()}
          aria-label="Refresh"
          disabled={isFetching}
          className="rounded-md p-2 hover:bg-muted"
        >
          <RefreshCw className={`h-4 w-4 ${isFetching ? 'animate-spin' : ''}`} />
        </button>
      </div>

      {/* Stats */}
      <div className="grid gap-4 md:grid-cols-4">
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm text-muted-foreground">Status</CardTitle>
          </CardHeader>
          <CardContent>
            <StatusIndicator status={data?.running ? 'running' : 'stopped'} />
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm text-muted-foreground">Total Tasks</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">{data?.total_tasks ?? 0}</div>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm text-muted-foreground">Active</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">{data?.active_tasks ?? 0}</div>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm text-muted-foreground">Max Concurrent</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">{data?.max_concurrent ?? '-'}</div>
          </CardContent>
        </Card>
      </div>

      {/* Task List */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Calendar className="h-4 w-4" /> Task Queue
          </CardTitle>
        </CardHeader>
        <CardContent>
          {tasks.length === 0 ? (
            <EmptyState
              icon={<Calendar className="h-8 w-8" />}
              title="No tasks"
              description="The scheduler queue is empty."
              className="py-6"
            />
          ) : (
            <div className="space-y-2">
              {tasks.map((task) => (
                <div
                  key={task.id}
                  className="flex items-center justify-between rounded-lg border p-3"
                >
                  <div className="flex items-center gap-3">
                    {task.status === 'running' ? (
                      <Loader2 className="h-4 w-4 animate-spin text-blue-500" />
                    ) : task.status === 'completed' ? (
                      <CheckCircle className="h-4 w-4 text-emerald-500" />
                    ) : task.status === 'failed' ? (
                      <Clock className="h-4 w-4 text-red-500" />
                    ) : (
                      <Clock className="h-4 w-4 text-amber-500" />
                    )}
                    <div>
                      <p className="font-medium text-sm">{task.id.slice(0, 12)}...</p>
                      {task.agent_id && (
                        <p className="text-xs text-muted-foreground">
                          Agent: {task.agent_id.slice(0, 8)}...
                        </p>
                      )}
                    </div>
                  </div>
                  <div className="flex items-center gap-2">
                    <Badge variant="outline">P{task.priority}</Badge>
                    <Badge
                      variant={
                        task.status === 'running'
                          ? 'success'
                          : task.status === 'failed'
                            ? 'destructive'
                            : 'secondary'
                      }
                    >
                      {task.status}
                    </Badge>
                  </div>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  )
}
