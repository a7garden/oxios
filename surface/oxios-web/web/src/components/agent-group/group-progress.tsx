import { cn } from '@/lib/utils'

interface Props {
  pct: number
  className?: string
  label?: string
}

export function GroupProgress({ pct, className, label }: Props) {
  return (
    <div className={cn('space-y-1', className)}>
      {label && (
        <div className="flex justify-between text-sm">
          <span>{label}</span>
          <span className="text-muted-foreground">{Math.round(pct)}%</span>
        </div>
      )}
      <div className="h-2 rounded-full bg-muted overflow-hidden">
        <div
          className={`h-full rounded-full transition-all ${
            pct >= 100 ? 'bg-emerald-500' : pct > 0 ? 'bg-primary' : 'bg-muted-foreground/30'
          }`}
          style={{ width: `${Math.min(100, pct)}%` }}
        />
      </div>
    </div>
  )
}
