import { statusDot } from '@/components/shared/status-palette'
import { cn } from '@/lib/utils'

interface StatusIndicatorProps {
  status: string
  className?: string
}

export function StatusIndicator({ status, className }: StatusIndicatorProps) {
  return (
    <div className={cn('flex items-center gap-2', className)}>
      <div
        className={cn('h-2 w-2 rounded-full', statusDot(status))}
        aria-hidden="true"
        title={status}
      />
      <span className="text-sm capitalize">{status}</span>
    </div>
  )
}
