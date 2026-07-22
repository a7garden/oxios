// Calendar render — event creation/update/listing summary.

import { Calendar } from 'lucide-react'
import type { ToolRenderComponent } from './registry'

export const CalendarRender: ToolRenderComponent = ({ args, result, isRunning }) => {
  const op = (args?.op ?? '') as string
  const title = (args?.title ?? '') as string
  const start = (args?.start ?? '') as string
  const resultStr = typeof result === 'string' ? result : ''

  return (
    <div className="space-y-1.5 text-sm">
      <div className="flex items-center gap-2 text-xs">
        <Calendar className="w-3.5 h-3.5 text-primary/70" />
        <span className="font-medium capitalize">{op || 'calendar'}</span>
        {title && <span className="text-muted-foreground truncate">{title}</span>}
      </div>
      {start && <div className="text-xs text-muted-foreground">{start}</div>}
      {isRunning && <div className="text-xs text-muted-foreground">Processing...</div>}
      {!isRunning && resultStr && (
        <div className="text-xs text-muted-foreground">{resultStr.slice(0, 300)}</div>
      )}
    </div>
  )
}
