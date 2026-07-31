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
          <span className="inline-block w-2 h-2 rounded-full bg-status-warning animate-pulse" />
          Editing...
        </div>
      ) : (
        <div className="space-y-1 text-xs font-mono">
          {oldText && (
            <div className="p-1.5 rounded bg-status-error-subtle border border-error-subtle-border text-status-error-on-subtle">
              <span>- {oldText.slice(0, 200)}</span>
            </div>
          )}
          {newText && (
            <div className="p-1.5 rounded bg-status-success-subtle border border-success-subtle-border text-status-success-on-subtle">
              <span>+ {newText.slice(0, 200)}</span>
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
