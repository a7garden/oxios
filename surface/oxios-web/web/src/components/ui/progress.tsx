import * as React from 'react'
import { cn } from '@/lib/utils'

interface ProgressProps extends React.HTMLAttributes<HTMLDivElement> {
  value?: number
  max?: number
  indicatorClassName?: string
}

const Progress = React.forwardRef<HTMLDivElement, ProgressProps>(
  ({ className, value = 0, max = 100, indicatorClassName, ...props }, ref) => (
    <div
      ref={ref}
      role="progressbar"
      aria-valuenow={value}
      aria-valuemin={0}
      aria-valuemax={max}
      className={cn('relative h-2 w-full overflow-hidden rounded-full bg-primary/20', className)}
      {...props}
    >
      <div
        className={cn(
          'h-full rounded-full bg-primary transition-all duration-300',
          indicatorClassName,
        )}
        style={{ width: `${Math.min(100, (value / max) * 100)}%` }}
      />
    </div>
  ),
)
Progress.displayName = 'Progress'

export { Progress }
