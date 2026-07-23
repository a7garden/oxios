// messages/components/ToolCallList — render ChatToolPayload[] as accordion.
//
// LobeHub analogue: Messages/AssistantGroup/Tool/index.tsx
//
// Each call becomes a collapsible card. Header = ToolInspector, body = the
// registered ToolRender (or DefaultToolRender fallback). Calls registered
// with custom inspectors replace the default header entirely.

import { ChevronRight } from 'lucide-react'
import { useState } from 'react'
import { DefaultToolRender, getToolRender } from '@/components/chat/tool-renders'
import { cn } from '@/lib/utils'
import type { ChatToolPayload } from '@/types/chat'
import { ToolInspector } from './ToolInspector'

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
        className="flex w-full items-center gap-1.5 px-2.5 py-1.5 text-left hover:bg-muted/50 transition-colors"
      >
        <ChevronRight
          className={cn(
            'w-3 h-3 shrink-0 text-muted-foreground transition-transform',
            open && 'rotate-90',
          )}
        />
        <ToolInspector call={call} />
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
            <p className="mt-2 text-xs text-destructive">{call.error.message ?? 'Tool error'}</p>
          )}
          {call.progress && isRunning && (
            <p className="mt-1 text-xs text-muted-foreground italic">{call.progress}</p>
          )}
        </div>
      )}
    </div>
  )
}
