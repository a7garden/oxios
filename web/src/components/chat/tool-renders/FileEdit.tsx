// FileEdit render — shows a simple diff-like view of the edit
import type { ToolRenderComponent } from './registry'

export const FileEditRender: ToolRenderComponent = ({ args, result, isRunning }) => {
  const path = (args?.path ?? args?.file_path ?? 'unknown') as string
  const oldText = (args?.old_text ?? args?.old_str ?? '') as string
  const newText = (args?.new_text ?? args?.new_str ?? '') as string

  return (
    <div className="space-y-2 text-sm">
      <div className="flex items-center gap-2 text-xs text-muted-foreground">
        <span className="font-mono bg-muted px-1.5 py-0.5 rounded">{path}</span>
      </div>
      {isRunning ? (
        <div className="flex items-center gap-2 text-muted-foreground">
          <span className="inline-block w-2 h-2 rounded-full bg-amber-500 animate-pulse" />
          Editing...
        </div>
      ) : (
        <div className="space-y-1 text-xs font-mono">
          {oldText && (
            <div className="p-1.5 rounded bg-red-50 dark:bg-red-950/20 border border-red-200 dark:border-red-900">
              <span className="text-red-600 dark:text-red-400">- {oldText.slice(0, 200)}</span>
            </div>
          )}
          {newText && (
            <div className="p-1.5 rounded bg-green-50 dark:bg-green-950/20 border border-green-200 dark:border-green-900">
              <span className="text-green-600 dark:text-green-400">+ {newText.slice(0, 200)}</span>
            </div>
          )}
          {result != null && typeof result === 'string' && (
            <div className="text-muted-foreground mt-1">{result}</div>
          )}
        </div>
      )}
    </div>
  )
}
