// A2A render — agent-to-agent delegation/send/query results.

import { Bot, Send, Users } from 'lucide-react'
import type { ToolRenderComponent } from './registry'

export const A2aDelegateRender: ToolRenderComponent = ({ args, result, isRunning }) => {
  const description = (args?.description ?? '') as string
  const capability = (args?.capability ?? '') as string
  const resultStr = typeof result === 'string' ? result : ''

  return (
    <div className="space-y-1.5 text-sm">
      <div className="flex items-center gap-2 text-xs">
        <Bot className="w-3.5 h-3.5 text-primary/70" />
        <span className="font-medium">Delegated</span>
        {capability && <span className="text-muted-foreground">{capability}</span>}
      </div>
      {description && <div className="text-xs truncate">{description}</div>}
      {isRunning && <div className="text-xs text-muted-foreground">Finding agent...</div>}
      {!isRunning && resultStr && (
        <div className="text-xs text-muted-foreground">{resultStr.slice(0, 200)}</div>
      )}
    </div>
  )
}

export const A2aSendRender: ToolRenderComponent = ({ args, isRunning }) => {
  const target = (args?.target_agent_id ?? '') as string
  const msgType = (args?.message_type ?? '') as string

  return (
    <div className="space-y-1.5 text-sm">
      <div className="flex items-center gap-2 text-xs">
        <Send className="w-3.5 h-3.5 text-muted-foreground" />
        <span className="font-medium">{msgType || 'message'}</span>
        {target && <span className="text-muted-foreground">→ {target.slice(0, 8)}</span>}
      </div>
      {isRunning && <div className="text-xs text-muted-foreground">Sending...</div>}
    </div>
  )
}

export const A2aQueryRender: ToolRenderComponent = ({ result, isRunning }) => {
  const resultStr = typeof result === 'string' ? result : JSON.stringify(result ?? '', null, 2)
  return (
    <div className="space-y-1.5 text-sm">
      <div className="flex items-center gap-2 text-xs">
        <Users className="w-3.5 h-3.5 text-muted-foreground" />
        <span className="font-medium">Agent Query</span>
      </div>
      {!isRunning && resultStr && (
        <pre className="p-2 rounded bg-muted text-xs overflow-x-auto max-h-48 whitespace-pre-wrap">
          {resultStr.slice(0, 1000)}
        </pre>
      )}
    </div>
  )
}
