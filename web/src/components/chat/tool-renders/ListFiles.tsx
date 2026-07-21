// ListFiles render — directory listing with file/dir indicators.

import { File, Folder } from 'lucide-react'
import type { ToolRenderComponent } from './registry'

interface ListEntry {
  name?: string
  path?: string
  is_dir?: boolean
  isDir?: boolean
  size?: number
}

export const ListFilesRender: ToolRenderComponent = ({ args, result, isRunning }) => {
  const path = (args?.path ?? args?.dir ?? '.') as string

  const entries: ListEntry[] = Array.isArray(result)
    ? (result as ListEntry[])
    : typeof result === 'string'
      ? result
          .split('\n')
          .filter(Boolean)
          .map((name) => ({ name, is_dir: name.endsWith('/') }))
      : []

  return (
    <div className="space-y-2 text-sm">
      <div className="text-xs text-muted-foreground font-mono">{path}</div>
      {isRunning ? (
        <div className="text-xs text-muted-foreground">Listing...</div>
      ) : entries.length === 0 ? (
        <div className="text-xs text-muted-foreground italic">Empty</div>
      ) : (
        <ul className="text-xs space-y-0.5 max-h-64 overflow-y-auto">
          {entries.slice(0, 100).map((e, i) => {
            const name = e.name ?? e.path ?? '<unknown>'
            const isDir = e.is_dir ?? e.isDir ?? name.endsWith('/')
            return (
              <li key={i} className="flex items-center gap-1.5">
                {isDir ? (
                  <Folder className="w-3 h-3 text-primary/70 shrink-0" />
                ) : (
                  <File className="w-3 h-3 text-muted-foreground shrink-0" />
                )}
                <span className="truncate font-mono">{name}</span>
              </li>
            )
          })}
          {entries.length > 100 && (
            <li className="text-muted-foreground italic">... {entries.length - 100} more</li>
          )}
        </ul>
      )}
    </div>
  )
}
