// messages/components/ToolCallList — render ChatToolPayload[] using the tool render registry.
//
// LobeHub analogue: Messages/AssistantGroup/Tool — each tool call is an
// AccordionItem with Inspector header + Detail body. Phase 2 keeps it simple:
// each call renders as a collapsible card, dispatching to the registered
// custom render or falling back to a JSON view. Phase 3 will expand into
// the full 4-tier registry (renders/inspectors/streamings/interventions).

import { ChevronRight } from 'lucide-react'
import { useState } from 'react'
import { cn } from '@/lib/utils'
import type { ChatToolPayload } from '@/types/chat'
import {
  DefaultToolRender,
  getToolRender,
} from '@/components/chat/tool-renders/registry'

interface ToolCallListProps {
  calls: ChatToolPayload[]
  /** Default expanded state for new tool calls. Defaults to false (collapsed). */
  defaultExpanded?: boolean
}

export function ToolCallList({ calls, defaultExpanded = false }: ToolCallListProps) {
  if (calls.length === 0) return null
  return (
    <div className="flex flex-col gap-1.5">
      {calls.map((c) => (
        <ToolCallCard key={c.id} call={c} defaultExpanded={defaultExpanded} />
      ))}
    </div>
  )
}

function ToolCallCard({
  call,
  defaultExpanded,
}: {
  call: ChatToolPayload
  defaultExpanded: boolean
}) {
  const [open, setOpen] = useState(defaultExpanded)
  const isRunning = call.status === 'loading'
  const isError = call.status === 'error'

  const Render = getToolRender(call.apiName)

  return (
    <div
      className={cn(
        'rounded-md border text-sm transition-colors',
        isError ? 'border-destructive/40 bg-destructive/5' : 'border-border bg-muted/30',
      )}
    >
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className="flex w-full items-center gap-2 px-2.5 py-1.5 text-left hover:bg-muted/50 transition-colors"
      >
        <ChevronRight
          className={cn(
            'w-3 h-3 shrink-0 text-muted-foreground transition-transform',
            open && 'rotate-90',
          )}
        />
        <StatusDot status={call.status} />
        <span className="font-mono text-xs font-medium">{call.apiName}</span>
        {call.durationMs !== undefined && (
          <span className="ml-auto text-2xs text-muted-foreground tabular-nums">
            {formatDuration(call.durationMs)}
          </span>
        )}
      </button>
      {open && (
        <div className="border-t border-border/60 px-2.5 py-2">
          {Render ? (
            <Render
              toolName={call.apiName}
              args={(call.arguments ?? {}) as Record<string, unknown>}
              result={call.result}
              isRunning={isRunning}
              durationMs={call.durationMs}
            />
          ) : (
            <DefaultToolRender
              toolName={call.apiName}
              args={(call.arguments ?? {}) as Record<string, unknown>}
              result={call.result}
              isRunning={isRunning}
              durationMs={call.durationMs}
            />
          )}
          {call.error && (
            <p className="mt-2 text-xs text-destructive">
              {call.error.message ?? 'Tool error'}
            </p>
          )}
          {call.progress && isRunning && (
            <p className="mt-1 text-xs text-muted-foreground italic">{call.progress}</p>
          )}
        </div>
      )}
    </div>
  )
}

function StatusDot({ status }: { status: ChatToolPayload['status'] }) {
  const cls =
    status === 'loading'
      ? 'bg-amber-500 animate-pulse'
      : status === 'error'
        ? 'bg-destructive'
        : status === 'aborted'
          ? 'bg-muted-foreground'
          : 'bg-emerald-500'
  return <span className={cn('inline-block w-2 h-2 rounded-full shrink-0', cls)} />
}

function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`
  return `${(ms / 1000).toFixed(1)}s`
}
