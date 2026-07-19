// ToolCallCard — uses shadcn Accordion + custom tool render registry
// Ported from LobeHub's AssistantGroup/Tool rendering pattern.

import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from '@/components/ui/accordion'
import { Wrench } from 'lucide-react'
import type { ToolCallSummary } from '@/types'
import { getToolRender, DefaultToolRender } from './tool-renders'

interface ToolCallCardProps {
  call: ToolCallSummary
  className?: string
}

export function ToolCallCard({ call, className }: ToolCallCardProps) {
  const toolName = call.tool_name ?? call.tool ?? 'unknown'
  const Render = getToolRender(toolName) ?? DefaultToolRender
  const durationStr =
    call.duration_ms >= 1000
      ? `${(call.duration_ms / 1000).toFixed(1)}s`
      : `${call.duration_ms}ms`

  // Parse args from input string
  let args: Record<string, unknown> = {}
  try {
    args = typeof call.input === 'string' ? JSON.parse(call.input) : (call.input ?? {})
  } catch {
    args = { raw: call.input }
  }

  return (
    <Accordion type="single" collapsible className={className}>
      <AccordionItem value="tool" className="border rounded-lg px-3">
        <AccordionTrigger className="py-2 hover:no-underline">
          <div className="flex items-center gap-2 text-sm min-w-0 flex-1">
            <Wrench className="w-3.5 h-3.5 text-muted-foreground shrink-0" />
            <span className="font-medium truncate">{toolName}</span>
            {durationStr && (
              <span className="text-xs text-muted-foreground ml-auto shrink-0">
                {durationStr}
              </span>
            )}
          </div>
        </AccordionTrigger>
        <AccordionContent>
          <Render
            toolName={toolName}
            args={args}
            result={call.output}
            isRunning={false}
            durationMs={call.duration_ms}
          />
        </AccordionContent>
      </AccordionItem>
    </Accordion>
  )
}
