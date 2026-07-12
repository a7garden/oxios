import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { CalendarClock, List, Pencil, Plus, Power, PowerOff, Timer, Trash2 } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { CronScheduleEditor } from '@/components/cron/cron-schedule-editor'
import { CronTimelineView } from '@/components/cron/cron-timeline-view'
import { EditCronDialog } from '@/components/cron/edit-cron-dialog'
import { EmptyState } from '@/components/shared/empty-state'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { RefreshButton } from '@/components/shared/refresh-button'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { api } from '@/lib/api-client'
import { DEFAULT_CRON } from '@/lib/cron-utils'
import { cn } from '@/lib/utils'
import type { CronJob } from '@/types'

export const Route = createFileRoute('/cron-jobs')({ component: CronJobsPage })

function CronJobsPage() {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const [showCreate, setShowCreate] = useState(false)
  const [editingJob, setEditingJob] = useState<CronJob | null>(null)
  const [name, setName] = useState('')
  const [schedule, setSchedule] = useState(DEFAULT_CRON)
  const [goal, setGoal] = useState('')
  const [viewMode, setViewMode] = useState<'list' | 'timeline'>('timeline')

  const { data, isLoading, isError, refetch, isFetching } = useQuery({
    queryKey: ['cron-jobs'],
    queryFn: async () => {
      const res = await api.get<{ jobs: CronJob[] }>('/api/cron-jobs')
      return Array.isArray(res?.jobs) ? res.jobs : []
    },
    refetchInterval: 10000,
  })

  const createMutation = useMutation({
    mutationFn: (job: { name: string; schedule: string; goal: string }) =>
      api.post('/api/cron-jobs', job),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['cron-jobs'] })
      setShowCreate(false)
      setName('')
      setSchedule(DEFAULT_CRON)
      setGoal('')
    },
  })

  const deleteMutation = useMutation({
    mutationFn: (id: string) => api.delete(`/api/cron-jobs/${id}`),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['cron-jobs'] }),
  })

  const toggleMutation = useMutation({
    mutationFn: ({ id, enabled }: { id: string; enabled: boolean }) =>
      api.post(`/api/cron-jobs/${id}/edit`, { enabled }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['cron-jobs'] }),
  })

  const updateMutation = useMutation({
    mutationFn: (job: { id: string; name: string; schedule: string; goal: string }) =>
      api.post(`/api/cron-jobs/${job.id}/edit`, job),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['cron-jobs'] })
      setEditingJob(null)
    },
  })

  if (isLoading) return <LoadingCards count={4} />
  if (isError) return <ErrorState onRetry={() => refetch()} />

  const jobs = Array.isArray(data) ? data : []

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">{t('cronJobs.title')}</h1>
          <p className="text-muted-foreground">{t('cronJobs.subtitle')}</p>
        </div>
        <div className="flex items-center gap-3">
          <div className="inline-flex gap-0.5 rounded-lg border bg-muted/50 p-0.5">
            <button
              type="button"
              onClick={() => setViewMode('list')}
              className={cn(
                'flex items-center gap-1.5 rounded-md px-2.5 py-1 text-xs font-medium transition-colors',
                viewMode === 'list'
                  ? 'bg-background text-foreground shadow-sm'
                  : 'text-muted-foreground hover:text-foreground',
              )}
            >
              <List className="h-3.5 w-3.5" />
              {t('cronJobs.timeline.viewList')}
            </button>
            <button
              type="button"
              onClick={() => setViewMode('timeline')}
              className={cn(
                'flex items-center gap-1.5 rounded-md px-2.5 py-1 text-xs font-medium transition-colors',
                viewMode === 'timeline'
                  ? 'bg-background text-foreground shadow-sm'
                  : 'text-muted-foreground hover:text-foreground',
              )}
            >
              <CalendarClock className="h-3.5 w-3.5" />
              {t('cronJobs.timeline.viewTimeline')}
            </button>
          </div>
          <RefreshButton onClick={() => refetch()} isFetching={isFetching} />
          <Button size="sm" onClick={() => setShowCreate(true)}>
            <Plus className="h-4 w-4 mr-1" /> {t('cronJobs.newJob')}
          </Button>
        </div>
      </div>

      {showCreate && (
        <Card>
          <CardHeader>
            <CardTitle>{t('cronJobs.createCronJob')}</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <Input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder={t('cronJobs.jobNamePlaceholder')}
            />
            <CronScheduleEditor value={schedule} onChange={setSchedule} />
            <span className="text-xs text-muted-foreground">{t('cronJobs.goalLabel')}</span>
            <Input
              value={goal}
              onChange={(e) => setGoal(e.target.value)}
              placeholder={t('cronJobs.goalPlaceholder')}
            />
            <div className="flex gap-2">
              <Button
                size="sm"
                onClick={() => createMutation.mutate({ name, schedule, goal })}
                disabled={!name || !schedule || !goal || createMutation.isPending}
              >
                {t('common.create')}
              </Button>
              <Button variant="ghost" size="sm" onClick={() => setShowCreate(false)}>
                {t('common.cancel')}
              </Button>
            </div>
          </CardContent>
        </Card>
      )}

      {jobs.length === 0 && !showCreate ? (
        <EmptyState
          icon={<Timer className="h-10 w-10" />}
          title={t('cronJobs.noCronJobs')}
          description={t('cronJobs.description')}
        />
      ) : viewMode === 'timeline' ? (
        <CronTimelineView
          jobs={jobs}
          onEdit={setEditingJob}
          onToggle={(job) => toggleMutation.mutate({ id: job.id, enabled: !job.enabled })}
          onDelete={(job) => deleteMutation.mutate(job.id)}
        />
      ) : (
        <div className="space-y-3">
          {jobs.map((job) => (
            <Card key={job.id} className={cn('transition-opacity', !job.enabled && 'opacity-60')}>
              <CardContent className="flex items-center justify-between p-4">
                <div>
                  <div className="font-medium flex items-center gap-2">
                    <Timer className="h-4 w-4" /> {job.name}
                    <Badge variant={job.enabled ? 'success' : 'secondary'}>
                      {job.enabled ? t('common.enabled') : t('common.disabled')}
                    </Badge>
                  </div>
                  <p className="text-sm text-muted-foreground mt-1">
                    <code className="text-xs bg-muted px-1 py-0.5 rounded">{job.schedule}</code>
                    {' → '}
                    <code className="text-xs bg-muted px-1 py-0.5 rounded">{job.goal}</code>
                  </p>
                  <div className="flex gap-4 text-xs text-muted-foreground mt-1">
                    {job.last_run && (
                      <span>
                        {t('cronJobs.lastRunLabel')} {new Date(job.last_run).toLocaleString()}
                      </span>
                    )}
                    {job.next_run && (
                      <span>
                        {t('cronJobs.nextRunLabel')} {new Date(job.next_run).toLocaleString()}
                      </span>
                    )}
                  </div>
                </div>
                <div className="flex gap-1">
                  <Button
                    variant="ghost"
                    size="icon"
                    onClick={() => setEditingJob(job)}
                    aria-label={t('common.edit')}
                  >
                    <Pencil className="h-4 w-4" />
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon"
                    onClick={() => toggleMutation.mutate({ id: job.id, enabled: !job.enabled })}
                    aria-label={job.enabled ? t('cronJobs.disableJob') : t('cronJobs.enableJob')}
                  >
                    {job.enabled ? <PowerOff className="h-4 w-4" /> : <Power className="h-4 w-4" />}
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon"
                    onClick={() => deleteMutation.mutate(job.id)}
                    aria-label={t('cronJobs.deleteJob')}
                  >
                    <Trash2 className="h-4 w-4 text-destructive" />
                  </Button>
                </div>
              </CardContent>
            </Card>
          ))}
        </div>
      )}
      <EditCronDialog
        job={editingJob}
        onOpenChange={(open) => !open && setEditingJob(null)}
        onSave={(patch) => {
          if (!editingJob) return
          updateMutation.mutate({
            id: editingJob.id,
            name: patch.name,
            schedule: patch.schedule,
            goal: patch.goal,
          })
        }}
        isPending={updateMutation.isPending}
      />
    </div>
  )
}
