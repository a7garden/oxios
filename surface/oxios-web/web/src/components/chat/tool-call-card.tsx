import { ChevronDown, ChevronRight, Wrench } from 'lucide-react'
import { useState } from 'react'
import { cn } from '@/lib/utils'
import type { ToolCallSummary } from '@/types'

interface ToolCallCardProps {
  call: ToolCallSummary
  className?: string
}

export function ToolCallCard({ call, className }: ToolCallCardProps) {
  const [expanded, setExpanded] = useState(false)
  const durationStr = call.duration_ms >= 1000
    ? `${(call.duration_ms / 1000).toFixed(1)}s`
    : `${call.duration_ms}ms`

  return (
    <div className={cn('rounded-lg border bg-muted/50 my-2', className)}>
      <button
        type="button"
        onClick={() => setExpanded(!expanded)}
        className="flex w-full items-center gap-2 px-3 py-2 text-sm"
      >
        {expanded ? <ChevronDown className="h-3.5 w-3.5" /> : <ChevronRight className="h-3.5 w-3.5" />}
        <Wrench className="h-3.5 w-3.5 text-muted-foreground" />
        <span className="font-medium">{call.tool_name}</span>
        <span className="ml-auto text-xs text-muted-foreground">{durationStr}</span>
      </button>
      {expanded && (
        <div className="border-t px-3 py-2 space-y-2">
          <div>
            <p className="text-xs font-medium text-muted-foreground mb-1">Input</p>
            <pre className="text-xs bg-background rounded p-2 overflow-x-auto whitespace-pre-wrap">{call.input}</pre>
          </div>
          <div>
            <p className="text-xs font-medium text-muted-foreground mb-1">Output</p>
            <pre className="text-xs bg-background rounded p-2 overflow-x-auto whitespace-pre-wrap max-h-48 overflow-y-auto">{call.output}</pre>
          </div>
        </div>
      )}
    </div>
  )
}
