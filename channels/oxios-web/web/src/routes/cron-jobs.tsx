import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { Plus, Power, PowerOff, RefreshCw, Timer, Trash2 } from 'lucide-react'
import { useState } from 'react'
import { ErrorState } from '@/components/shared/error-state'
import { EmptyState } from '@/components/shared/empty-state'
import { LoadingCards } from '@/components/shared/loading'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { api } from '@/lib/api-client'
import type { CronJob } from '@/types'

export const Route = createFileRoute('/cron-jobs')({ component: CronJobsPage })

function CronJobsPage() {
  const queryClient = useQueryClient()
  const [showCreate, setShowCreate] = useState(false)
  const [name, setName] = useState('')
  const [schedule, setSchedule] = useState('')
  const [command, setCommand] = useState('')

  const { data, isLoading, isError, refetch, isFetching } = useQuery({
    queryKey: ['cron-jobs'],
    queryFn: () => api.get<CronJob[]>('/api/cron-jobs'),
    refetchInterval: 10000,
  })

  const createMutation = useMutation({
    mutationFn: (job: { name: string; schedule: string; command: string }) =>
      api.post('/api/cron-jobs', job),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['cron-jobs'] })
      setShowCreate(false)
      setName('')
      setSchedule('')
      setCommand('')
    },
  })

  const deleteMutation = useMutation({
    mutationFn: (id: string) => api.delete(`/api/cron-jobs/${id}`),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['cron-jobs'] }),
  })

  const toggleMutation = useMutation({
    mutationFn: ({ id, enabled }: { id: string; enabled: boolean }) =>
      api.put(`/api/cron-jobs/${id}`, { enabled }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['cron-jobs'] }),
  })

  if (isLoading) return <LoadingCards count={4} />
  if (isError) return <ErrorState onRetry={() => refetch()} />

  const jobs = data ?? []

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Cron Jobs</h1>
          <p className="text-muted-foreground">Scheduled task management</p>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" size="sm" onClick={() => refetch()} disabled={isFetching}>
            <RefreshCw className={`h-4 w-4 mr-1 ${isFetching ? 'animate-spin' : ''}`} /> Refresh
          </Button>
          <Button size="sm" onClick={() => setShowCreate(true)}>
            <Plus className="h-4 w-4 mr-1" /> New Job
          </Button>
        </div>
      </div>

      {showCreate && (
        <Card>
          <CardHeader>
            <CardTitle>Create Cron Job</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <Input value={name} onChange={(e) => setName(e.target.value)} placeholder="Job name" />
            <Input
              value={schedule}
              onChange={(e) => setSchedule(e.target.value)}
              placeholder="Cron schedule (e.g. */5 * * * *)"
            />
            <Input
              value={command}
              onChange={(e) => setCommand(e.target.value)}
              placeholder="Command to execute"
            />
            <div className="flex gap-2">
              <Button
                size="sm"
                onClick={() => createMutation.mutate({ name, schedule, command })}
                disabled={!name || !schedule || !command || createMutation.isPending}
              >
                Create
              </Button>
              <Button variant="ghost" size="sm" onClick={() => setShowCreate(false)}>
                Cancel
              </Button>
            </div>
          </CardContent>
        </Card>
      )}

      {jobs.length === 0 && !showCreate ? (
        <EmptyState
          icon={<Timer className="h-10 w-10" />}
          title="No cron jobs"
          description="Create scheduled tasks to automate recurring work."
        />
      ) : (
        <div className="space-y-3">
          {jobs.map((job) => (
            <Card key={job.id}>
              <CardContent className="flex items-center justify-between p-4">
                <div>
                  <p className="font-medium flex items-center gap-2">
                    <Timer className="h-4 w-4" /> {job.name}
                    <Badge variant={job.enabled ? 'success' : 'secondary'}>
                      {job.enabled ? 'Enabled' : 'Disabled'}
                    </Badge>
                  </p>
                  <p className="text-sm text-muted-foreground mt-1">
                    <code className="text-xs bg-muted px-1 py-0.5 rounded">{job.schedule}</code>
                    {' → '}
                    <code className="text-xs bg-muted px-1 py-0.5 rounded">{job.command}</code>
                  </p>
                  <div className="flex gap-4 text-xs text-muted-foreground mt-1">
                    {job.last_run && <span>Last: {new Date(job.last_run).toLocaleString()}</span>}
                    {job.next_run && <span>Next: {new Date(job.next_run).toLocaleString()}</span>}
                  </div>
                </div>
                <div className="flex gap-1">
                  <Button
                    variant="ghost"
                    size="icon"
                    onClick={() => toggleMutation.mutate({ id: job.id, enabled: !job.enabled })}
                    aria-label={job.enabled ? 'Disable job' : 'Enable job'}
                  >
                    {job.enabled ? <PowerOff className="h-4 w-4" /> : <Power className="h-4 w-4" />}
                  </Button>
                  <Button variant="ghost" size="icon" onClick={() => deleteMutation.mutate(job.id)} aria-label="Delete job">
                    <Trash2 className="h-4 w-4 text-destructive" />
                  </Button>
                </div>
              </CardContent>
            </Card>
          ))}
        </div>
      )}
    </div>
  )
}
