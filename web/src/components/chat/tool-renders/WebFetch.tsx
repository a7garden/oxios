// WebFetch render — fetched URL with title + content preview.

import { Globe } from 'lucide-react'
import type { ToolRenderComponent } from './registry'

interface FetchResult {
  url?: string
  title?: string
  content?: string
  text?: string
  status?: number
}

export const WebFetchRender: ToolRenderComponent = ({ args, result, isRunning }) => {
  const url = (args?.url ?? args?.uri ?? '') as string
  const parsed: FetchResult =
    typeof result === 'string' ? (tryJson(result) ?? { content: result }) : (result as FetchResult)

  return (
    <div className="space-y-2 text-sm">
      <div className="flex items-center gap-2 text-xs">
        <Globe className="w-3.5 h-3.5 text-muted-foreground shrink-0" />
        <a
          href={parsed.url ?? url}
          target="_blank"
          rel="noreferrer"
          className="text-primary hover:underline truncate"
        >
          {parsed.title ?? url}
        </a>
        {parsed.status && <span className="text-muted-foreground/60 ml-auto">{parsed.status}</span>}
      </div>
      {isRunning ? (
        <div className="text-xs text-muted-foreground">Fetching...</div>
      ) : (
        <pre className="p-2 rounded bg-muted text-xs overflow-x-auto max-h-64 whitespace-pre-wrap">
          {(parsed.content ?? parsed.text ?? '').slice(0, 1000)}
          {(parsed.content ?? parsed.text ?? '').length > 1000 ? '…' : ''}
        </pre>
      )}
    </div>
  )
}

function tryJson(s: string): FetchResult | null {
  try {
    return JSON.parse(s) as FetchResult
  } catch {
    return null
  }
}
