// Task page — list + create + manage tasks (RFC-043)

import { createFileRoute } from '@tanstack/react-router'
import { useState } from 'react'
import { useTasks, useCreateTask, useDeleteTask, useUpdateTaskStatus, useRunTask } from '@/hooks/use-tasks'
import { EmptyState } from '@/components/shared/empty-state'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { PageHeader } from '@/components/shared/page-header'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Textarea } from '@/components/ui/textarea'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogTrigger } from '@/components/ui/dialog'
import { TASK_STATUS_META, TASK_STATUSES, type Task, type TaskStatus } from '@/types/task'
import { Plus, Trash2, Play, Clock } from 'lucide-react'
import { cn } from '@/lib/utils'

export const Route = createFileRoute('/tasks')({ component: TasksPage })

function TasksPage() {
  const { data, isLoading, isError, refetch } = useTasks()
  const [showCreate, setShowCreate] = useState(false)
  const [statusFilter, setStatusFilter] = useState<TaskStatus | 'all'>('all')

  if (isLoading) return <LoadingCards count={4} />
  if (isError) return <ErrorState onRetry={() => refetch()} />

  const allTasks = data?.tasks ?? []
  const tasks = statusFilter === 'all' ? allTasks : allTasks.filter((t) => t.status === statusFilter)

  return (
    <div className="space-y-6">
      <PageHeader
        title="Tasks"
        subtitle="Agent task lifecycle management"
        actions={
          <Dialog open={showCreate} onOpenChange={setShowCreate}>
            <DialogTrigger asChild>
              <Button size="sm" className="gap-1.5">
                <Plus className="h-3.5 w-3.5" />
                New Task
              </Button>
            </DialogTrigger>
            <CreateTaskDialog onClose={() => setShowCreate(false)} />
          </Dialog>
        }
      />

      {/* Status filter chips */}
      <div className="flex items-center gap-1 overflow-x-auto pb-1">
        <StatusChip label="All" count={allTasks.length} active={statusFilter === 'all'} onClick={() => setStatusFilter('all')} />
        {TASK_STATUSES.map((status) => {
          const count = allTasks.filter((t) => t.status === status).length
          if (count === 0) return null
          const meta = TASK_STATUS_META[status]
          return (
            <StatusChip
              key={status}
              label={meta.label}
              count={count}
              active={statusFilter === status}
              onClick={() => setStatusFilter(status)}
              colorClass={meta.color}
            />
          )
        })}
      </div>

      {/* Task list */}
      {tasks.length === 0 ? (
        <EmptyState
          icon={<Plus className="h-8 w-8" />}
          title="No tasks yet"
          description="Create a task to schedule recurring agent work"
          action={<Button size="sm" onClick={() => setShowCreate(true)}>Create Task</Button>}
        />
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
          {tasks.map((task) => (
            <TaskCard key={task.id} task={task} />
          ))}
        </div>
      )}
    </div>
  )
}

// ── Status chip ──

function StatusChip({
  label,
  count,
  active,
  onClick,
  colorClass,
}: {
  label: string
  count: number
  active: boolean
  onClick: () => void
  colorClass?: string
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        'flex items-center gap-1.5 px-3 py-1.5 rounded-full text-sm whitespace-nowrap transition-colors',
        active ? 'bg-primary text-primary-foreground' : 'bg-muted text-muted-foreground hover:bg-muted/80',
      )}
    >
      <span className={cn(!active && colorClass)}>{label}</span>
      <span className={cn(
        'text-xs px-1.5 py-0.5 rounded-full',
        active ? 'bg-primary-foreground/20' : 'bg-background/50',
      )}>
        {count}
      </span>
    </button>
  )
}

// ── Task card ──

function TaskCard({ task }: { task: Task }) {
  const deleteMutation = useDeleteTask()
  const statusMutation = useUpdateTaskStatus()
  const runMutation = useRunTask()
  const meta = TASK_STATUS_META[task.status]

  const handleRun = () => runMutation.mutate({ id: task.id })
  const handleDelete = () => deleteMutation.mutate(task.id)
  const handleComplete = () => statusMutation.mutate({ id: task.id, status: 'completed' })

  return (
    <div className="flex flex-col rounded-xl border bg-card p-4 hover:border-primary/30 hover:shadow-sm transition-all">
      {/* Header */}
      <div className="flex items-start justify-between gap-2 mb-2">
        <div className="min-w-0 flex-1">
          <h3 className="text-sm font-semibold truncate">{task.name}</h3>
          <p className="text-xs text-muted-foreground font-mono truncate">{task.identifier}</p>
        </div>
        <span className={cn('text-xs px-2 py-0.5 rounded-full font-medium shrink-0', meta.bgColor, meta.color)}>
          {meta.label}
        </span>
      </div>

      {/* Description */}
      {task.description && (
        <p className="text-xs text-muted-foreground line-clamp-2 mb-2">{task.description}</p>
      )}

      {/* Schedule info */}
      {task.automationMode && (
        <div className="flex items-center gap-1 text-xs text-muted-foreground mb-2">
          <Clock className="h-3 w-3" />
          <span>
            {task.automationMode === 'schedule'
              ? task.schedulePattern ?? 'cron'
              : `every ${task.heartbeatIntervalSecs ?? 0}s`}
          </span>
          {task.executionCount > 0 && (
            <span className="text-muted-foreground/60 ml-auto">
              {task.executionCount} runs
            </span>
          )}
        </div>
      )}

      {/* Actions */}
      <div className="flex items-center gap-1 mt-auto pt-2">
        {task.status !== 'completed' && task.status !== 'running' && (
          <Button size="sm" variant="ghost" className="h-7 text-xs gap-1" onClick={handleRun} disabled={runMutation.isPending}>
            <Play className="h-3 w-3" />
            Run
          </Button>
        )}
        {task.status === 'running' && (
          <Button size="sm" variant="ghost" className="h-7 text-xs gap-1" onClick={handleComplete}>
            Complete
          </Button>
        )}
        <Button size="sm" variant="ghost" className="h-7 text-xs text-muted-foreground hover:text-destructive ml-auto" onClick={handleDelete} disabled={deleteMutation.isPending}>
          <Trash2 className="h-3 w-3" />
        </Button>
      </div>
    </div>
  )
}

// ── Create dialog ──

function CreateTaskDialog({ onClose }: { onClose: () => void }) {
  const createMutation = useCreateTask()
  const [name, setName] = useState('')
  const [instruction, setInstruction] = useState('')
  const [description, setDescription] = useState('')

  const handleSubmit = () => {
    if (!name.trim() || !instruction.trim()) return
    createMutation.mutate(
      { name: name.trim(), instruction: instruction.trim(), description: description.trim() || undefined },
      { onSuccess: onClose },
    )
  }

  return (
    <DialogContent className="sm:max-w-md">
      <DialogHeader>
        <DialogTitle>Create Task</DialogTitle>
      </DialogHeader>
      <div className="space-y-3">
        <div>
          <label className="text-sm font-medium mb-1 block">Name</label>
          <Input value={name} onChange={(e) => setName(e.target.value)} placeholder="Weekly Font Recommendations" />
        </div>
        <div>
          <label className="text-sm font-medium mb-1 block">Description (optional)</label>
          <Input value={description} onChange={(e) => setDescription(e.target.value)} placeholder="Every Wednesday, get 3 font pairings" />
        </div>
        <div>
          <label className="text-sm font-medium mb-1 block">Instruction</label>
          <Textarea
            value={instruction}
            onChange={(e) => setInstruction(e.target.value)}
            placeholder="You are a design curator. Provide 3 font pairings..."
            rows={4}
          />
        </div>
        <div className="flex justify-end gap-2 pt-2">
          <Button variant="ghost" size="sm" onClick={onClose}>Cancel</Button>
          <Button size="sm" onClick={handleSubmit} disabled={!name.trim() || !instruction.trim() || createMutation.isPending}>
            {createMutation.isPending ? 'Creating...' : 'Create Task'}
          </Button>
        </div>
      </div>
    </DialogContent>
  )
}
