// Glob render — file pattern matching results as a list.

import { FileSearch } from 'lucide-react'
import type { ToolRenderComponent } from './registry'

export const GlobRender: ToolRenderComponent = ({ args, result, isRunning }) => {
  const pattern = (args?.pattern ?? args?.glob ?? '') as string
  const path = (args?.path ?? args?.cwd ?? '.') as string

  const files = Array.isArray(result)
    ? (result as unknown[])
    : typeof result === 'string'
      ? result.split('\n').filter(Boolean)
      : []

  return (
    <div className="space-y-2 text-sm">
      <div className="flex items-center gap-2 text-xs text-muted-foreground">
        <FileSearch className="w-3.5 h-3.5" />
        <span className="font-mono bg-muted px-1.5 py-0.5 rounded">{pattern}</span>
        <span className="text-muted-foreground/60">in {path}</span>
      </div>
      {isRunning ? (
        <div className="text-xs text-muted-foreground">Searching...</div>
      ) : files.length === 0 ? (
        <div className="text-xs text-muted-foreground italic">No matches</div>
      ) : (
        <ul className="text-xs font-mono space-y-0.5 max-h-64 overflow-y-auto">
          {files.slice(0, 100).map((f, i) => (
            <li key={i} className="truncate hover:text-foreground transition-colors">
              {String(f)}
            </li>
          ))}
          {files.length > 100 && (
            <li className="text-muted-foreground italic">... {files.length - 100} more</li>
          )}
        </ul>
      )}
    </div>
  )
}
