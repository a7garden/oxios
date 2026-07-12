import { Brain, ChevronDown, ChevronRight, Clock, Lightbulb, Wrench } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Badge } from '@/components/ui/badge'
import type { AgentTraceStep } from '@/types/agent'

interface TraceStepProps {
  step: AgentTraceStep
}

/** Icon + color for a trace step kind. */
function kindVisual(kind: AgentTraceStep['kind']) {
  switch (kind) {
    case 'memory':
      return { icon: <Brain className="h-3.5 w-3.5 text-info" />, badgeClass: 'text-info' }
    case 'reasoning':
      return {
        icon: <Lightbulb className="h-3.5 w-3.5 text-message-task" />,
        badgeClass: 'text-message-task',
      }
    default:
      return { icon: <Wrench className="h-3.5 w-3.5 text-muted-foreground" />, badgeClass: '' }
  }
}

/** Human-readable label for a kind. */
function kindLabel(
  kind: AgentTraceStep['kind'],
  t: ReturnType<typeof useTranslation>['t'],
): string | null {
  switch (kind) {
    case 'memory':
      return t('agents.memoryRecall')
    case 'reasoning':
      return t('agents.reasoning')
    default:
      return null
  }
}

export function TraceStepCard({ step }: TraceStepProps) {
  const [expanded, setExpanded] = useState(false)
  const { t } = useTranslation()
  const durationSec = (step.duration_ms / 1000).toFixed(1)
  const statusColor =
    step.status === 'completed'
      ? 'bg-success'
      : step.status === 'failed'
        ? 'bg-error'
        : 'bg-warning'
  const kind = step.kind
  const visual = kindVisual(kind)
  const kLabel = kindLabel(kind, t)
  const badgeText = kLabel ?? step.tool_name ?? step.action
  return (
    <div className="border rounded-lg">
      <button
        type="button"
        className="flex items-center gap-3 p-3 w-full text-left hover:bg-muted/50"
        onClick={() => setExpanded(!expanded)}
      >
        <div className={`w-2 h-2 rounded-full ${statusColor}`} />
        {visual.icon}
        <Badge
          variant="outline"
          className={`text-xs font-mono truncate min-w-0 max-w-[180px] ${visual.badgeClass}`}
        >
          {badgeText}
        </Badge>
        <span className="text-xs text-muted-foreground flex items-center gap-1">
          <Clock className="h-3 w-3" /> {durationSec}s
        </span>
        <div className="flex-1" />
        {expanded ? <ChevronDown className="h-4 w-4" /> : <ChevronRight className="h-4 w-4" />}
      </button>
      {expanded && (
        <div className="border-t px-3 py-2 space-y-2 bg-muted/30">
          <div>
            <p className="text-xs font-medium text-muted-foreground mb-1">
              {t('agents.inputLabel')}
            </p>
            <pre className="text-xs bg-muted rounded p-2 overflow-x-auto whitespace-pre-wrap max-h-48 overflow-y-auto">
              {typeof step.input === 'string' ? step.input : JSON.stringify(step.input, null, 2)}
            </pre>
          </div>
          <div>
            <p className="text-xs font-medium text-muted-foreground mb-1">
              {t('agents.outputLabel')}
            </p>
            <pre className="text-xs bg-muted rounded p-2 overflow-x-auto whitespace-pre-wrap max-h-48">
              {typeof step.output === 'string' ? step.output : JSON.stringify(step.output, null, 2)}
            </pre>
          </div>
        </div>
      )}
    </div>
  )
}
