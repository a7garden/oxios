import { useState } from 'react'
import { ChevronDown, ChevronRight, Clock } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import type { AgentTraceStep } from '@/types/agent'

interface TraceStepProps {
  step: AgentTraceStep
}

export function TraceStepCard({ step }: TraceStepProps) {
  const [expanded, setExpanded] = useState(false)
  const durationSec = (step.duration_ms / 1000).toFixed(1)
  const statusColor =
    step.status === 'completed'
      ? 'bg-success'
      : step.status === 'failed'
        ? 'bg-error'
        : 'bg-warning'

  return (
    <div className="border rounded-lg">
      <div
        className="flex items-center gap-3 p-3 cursor-pointer hover:bg-muted/50"
        onClick={() => setExpanded(!expanded)}
        role="button"
        tabIndex={0}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') setExpanded(!expanded)
        }}
      >
        <div className={`w-2 h-2 rounded-full ${statusColor}`} />
        <Badge variant="outline" className="text-xs font-mono truncate min-w-0 max-w-[180px]">
          {step.tool_name || step.action}
        </Badge>
        <span className="text-xs text-muted-foreground flex items-center gap-1">
          <Clock className="h-3 w-3" /> {durationSec}s
        </span>
        <div className="flex-1" />
        {expanded ? <ChevronDown className="h-4 w-4" /> : <ChevronRight className="h-4 w-4" />}
      </div>
      {expanded && (
        <div className="border-t px-3 py-2 space-y-2 bg-muted/30">
          <div>
            <p className="text-xs font-medium text-muted-foreground mb-1">Input</p>
            <pre className="text-xs bg-muted rounded p-2 overflow-x-auto whitespace-pre-wrap max-h-48 overflow-y-auto">
              {typeof step.input === 'string' ? step.input : JSON.stringify(step.input, null, 2)}
            </pre>
          </div>
          <div>
            <p className="text-xs font-medium text-muted-foreground mb-1">Output</p>
            <pre className="text-xs bg-muted rounded p-2 overflow-x-auto whitespace-pre-wrap max-h-48">
              {typeof step.output === 'string'
                ? step.output
                : JSON.stringify(step.output, null, 2)}
            </pre>
          </div>
        </div>
      )}
    </div>
  )
}
