import { cn } from '@/lib/utils'

interface StatusIndicatorProps {
  status: string
  className?: string
}

const statusColors: Record<string, string> = {
  running: 'bg-emerald-500',
  active: 'bg-emerald-500',
  idle: 'bg-amber-500',
  pending: 'bg-amber-500',
  stopped: 'bg-zinc-400',
  archived: 'bg-zinc-400',
  error: 'bg-destructive',
  failed: 'bg-destructive',
  rejected: 'bg-destructive',
}

export function StatusIndicator({ status, className }: StatusIndicatorProps) {
  return (
    <div className={cn('flex items-center gap-2', className)}>
      <div
        className={cn('h-2 w-2 rounded-full', statusColors[status] ?? 'bg-zinc-400')}
        aria-hidden="true"
      />
      <span className="text-sm capitalize">{status}</span>
    </div>
  )
}
