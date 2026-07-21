// messages/components/ToolInspector — header row for a tool call.
//
// LobeHub analogue: Messages/AssistantGroup/Tool/Inspector/index.tsx
//
// Shows: status indicator, tool icon, name, short args preview, duration,
// progress text (when running). Custom inspectors per tool can be registered
// via registerToolInspector — they replace this default entirely.

import { memo } from 'react'
import { cn } from '@/lib/utils'
import type { ChatToolPayload } from '@/types/chat'
import { getToolInspector } from '@/components/chat/tool-renders'

interface ToolInspectorProps {
  call: ChatToolPayload
}

function ToolInspectorImpl({ call }: ToolInspectorProps) {
  // Allow per-tool custom inspector.
  const Custom = getToolInspector(call.apiName)
  if (Custom) return <Custom call={call} />

  const isRunning = call.status === 'loading'
  const isError = call.status === 'error'

  return (
    <div className="flex items-center gap-2 min-w-0">
      <StatusIndicator status={call.status} />
      <span
        className={cn(
          'font-mono text-xs font-medium truncate',
          isError && 'text-destructive',
        )}
      >
        {call.apiName}
      </span>
      <ArgsPreview call={call} />
      {call.durationMs !== undefined && !isRunning && (
        <span className="ml-auto text-2xs text-muted-foreground tabular-nums shrink-0">
          {formatDuration(call.durationMs)}
        </span>
      )}
    </div>
  )
}

export const ToolInspector = memo(ToolInspectorImpl)

// ── Internals ──

function StatusIndicator({ status }: { status: ChatToolPayload['status'] }) {
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

function ArgsPreview({ call }: { call: ChatToolPayload }) {
  const args = call.arguments
  if (!args || typeof args !== 'object') return null

  // Pick the most informative single field to preview inline.
  const candidateFields = ['path', 'file_path', 'command', 'cmd', 'pattern', 'query', 'url', 'uri']
  for (const field of candidateFields) {
    const v = (args as Record<string, unknown>)[field]
    if (typeof v === 'string' && v) {
      const truncated = v.length > 60 ? v.slice(0, 57) + '...' : v
      return (
        <span className="text-2xs text-muted-foreground truncate font-mono">{truncated}</span>
      )
    }
  }
  return null
}

function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`
  return `${(ms / 1000).toFixed(1)}s`
}
