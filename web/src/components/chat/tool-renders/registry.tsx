// Tool render registry — pluggable custom renders for tool results (ported from LobeHub)
//
// LobeHub original: /tmp/lobehub/packages/builtin-tools/src/register.ts
// Each tool can register a custom React component to render its result.
// Falls back to DefaultToolRender for unregistered tools.

import type { ComponentType } from 'react'

// ── Types ──

export interface ToolRenderProps {
  toolName: string
  args: Record<string, unknown>
  result: unknown
  isRunning: boolean
  durationMs?: number
}

export type ToolRenderComponent = ComponentType<ToolRenderProps>

// ── Registry ──

const registry = new Map<string, ToolRenderComponent>()

export function registerToolRender(toolName: string, component: ToolRenderComponent): void {
  registry.set(toolName, component)
}

export function getToolRender(toolName: string): ToolRenderComponent | undefined {
  return registry.get(toolName)
}

// ── Default fallback ──

export function DefaultToolRender({ args, result, isRunning }: ToolRenderProps) {
  return (
    <div className="space-y-2 text-sm">
      {isRunning && (
        <div className="flex items-center gap-2 text-muted-foreground">
          <span className="inline-block w-2 h-2 rounded-full bg-amber-500 animate-pulse" />
          Running...
        </div>
      )}
      {args && Object.keys(args).length > 0 && (
        <details className="group">
          <summary className="cursor-pointer text-xs text-muted-foreground hover:text-foreground transition-colors">
            Input
          </summary>
          <pre className="mt-1 p-2 rounded bg-muted text-xs overflow-x-auto max-h-48">
            {JSON.stringify(args, null, 2)}
          </pre>
        </details>
      )}
      {result != null && !isRunning && (
        <details className="group" open>
          <summary className="cursor-pointer text-xs text-muted-foreground hover:text-foreground transition-colors">
            Output
          </summary>
          <pre className="mt-1 p-2 rounded bg-muted text-xs overflow-x-auto max-h-96 whitespace-pre-wrap">
            {typeof result === 'string' ? result : JSON.stringify(result, null, 2)}
          </pre>
        </details>
      )}
    </div>
  )
}
