// Grep render — search hits with file:line context.

import { Search } from 'lucide-react'
import type { ToolRenderComponent } from './registry'

interface GrepHit {
  path?: string
  file?: string
  line?: number
  ln?: number
  text?: string
  content?: string
  match?: string
}

export const GrepRender: ToolRenderComponent = ({ args, result, isRunning }) => {
  const pattern = (args?.pattern ?? args?.regex ?? args?.query ?? '') as string

  const hits: GrepHit[] = Array.isArray(result)
    ? (result as GrepHit[])
    : typeof result === 'string'
      ? parseGrepString(result)
      : []

  return (
    <div className="space-y-2 text-sm">
      <div className="flex items-center gap-2 text-xs text-muted-foreground">
        <Search className="w-3.5 h-3.5" />
        <span className="font-mono bg-muted px-1.5 py-0.5 rounded">{pattern}</span>
        <span className="text-muted-foreground/60">{hits.length} hits</span>
      </div>
      {isRunning ? (
        <div className="text-xs text-muted-foreground">Searching...</div>
      ) : hits.length === 0 ? (
        <div className="text-xs text-muted-foreground italic">No matches</div>
      ) : (
        <ul className="text-xs font-mono space-y-1 max-h-64 overflow-y-auto">
          {hits.slice(0, 50).map((h, i) => {
            const file = h.path ?? h.file ?? '<unknown>'
            const line = h.line ?? h.ln
            const text = h.text ?? h.content ?? h.match ?? ''
            return (
              <li key={i} className="truncate">
                <span className="text-muted-foreground">{file}</span>
                {line != null && <span className="text-muted-foreground/60">:{line}</span>}
                <span className="ml-2 text-foreground/80">{String(text).slice(0, 120)}</span>
              </li>
            )
          })}
          {hits.length > 50 && (
            <li className="text-muted-foreground italic">... {hits.length - 50} more</li>
          )}
        </ul>
      )}
    </div>
  )
}

function parseGrepString(s: string): GrepHit[] {
  // Best-effort parse of common grep output formats:
  //   path:line:content
  //   path:content
  return s
    .split('\n')
    .filter(Boolean)
    .map((line) => {
      const m = line.match(/^([^:]+):(\d+):(.*)$/)
      if (m) return { path: m[1], line: Number(m[2]), text: m[3] }
      const m2 = line.match(/^([^:]+):(.*)$/)
      if (m2) return { path: m2[1], text: m2[2] }
      return { text: line }
    })
}
