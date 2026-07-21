// Tool render registry — 4-tier pluggable components per tool (LobeHub-aligned).
//
// LobeHub analogue: packages/builtin-tools/src/{renders,inspectors,streamings,interventions}.ts
//
// Phase 3 (2026-07-21): introduces the 4-tier TYPE system. Phase 3 ships
// renders + inspectors; streamings and interventions are typed for forward
// compat but not yet populated.
//
// See docs/designs/2026-07-21-lobehub-chat-port-design.md §7 Phase 3.

import type { ComponentType } from 'react'
import type { ChatToolPayload } from '@/types/chat'

// ── Prop types ──

export interface ToolRenderProps {
  toolName: string
  args: Record<string, unknown>
  result: unknown
  isRunning: boolean
  durationMs?: number
}
export type ToolRenderComponent = ComponentType<ToolRenderProps>

export interface ToolInspectorProps {
  call: ChatToolPayload
}
export type ToolInspectorComponent = ComponentType<ToolInspectorProps>

export interface ToolStreamingProps {
  toolName: string
  progress?: string
  args: Record<string, unknown>
}
export type ToolStreamingComponent = ComponentType<ToolStreamingProps>

export interface ToolInterventionProps {
  call: ChatToolPayload
  onApprove: () => void
  onReject: () => void
}
export type ToolInterventionComponent = ComponentType<ToolInterventionProps>

// ── Registries (4 tiers) ──

const renders = new Map<string, ToolRenderComponent>()
const inspectors = new Map<string, ToolInspectorComponent>()
const streamings = new Map<string, ToolStreamingComponent>()
const interventions = new Map<string, ToolInterventionComponent>()

export function registerToolRender(toolName: string, component: ToolRenderComponent): void {
  renders.set(toolName, component)
}
export function registerToolInspector(toolName: string, component: ToolInspectorComponent): void {
  inspectors.set(toolName, component)
}
export function registerToolStreaming(toolName: string, component: ToolStreamingComponent): void {
  streamings.set(toolName, component)
}
export function registerToolIntervention(
  toolName: string,
  component: ToolInterventionComponent,
): void {
  interventions.set(toolName, component)
}

export function getToolRender(toolName: string): ToolRenderComponent | undefined {
  return renders.get(toolName)
}
export function getToolInspector(toolName: string): ToolInspectorComponent | undefined {
  return inspectors.get(toolName)
}
export function getToolStreaming(toolName: string): ToolStreamingComponent | undefined {
  return streamings.get(toolName)
}
export function getToolIntervention(toolName: string): ToolInterventionComponent | undefined {
  return interventions.get(toolName)
}

// ── Default fallbacks ──

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
