// FileRead render — displays file content with path and syntax hint
import type { ToolRenderComponent } from './registry'

export const FileReadRender: ToolRenderComponent = ({ args, result, isRunning }) => {
  const path = (args?.path ?? args?.file_path ?? 'unknown') as string

  return (
    <div className="space-y-2 text-sm">
      <div className="flex items-center gap-2 text-xs text-muted-foreground">
        <span className="font-mono bg-muted px-1.5 py-0.5 rounded">{path}</span>
      </div>
      {isRunning ? (
        <div className="flex items-center gap-2 text-muted-foreground">
          <span className="inline-block w-2 h-2 rounded-full bg-amber-500 animate-pulse" />
          Reading...
        </div>
      ) : result != null ? (
        <pre className="p-2 rounded bg-muted text-xs overflow-x-auto max-h-96 whitespace-pre-wrap font-mono leading-relaxed">
          {typeof result === 'string' ? truncate(result, 8000) : JSON.stringify(result, null, 2)}
        </pre>
      ) : null}
    </div>
  )
}

function truncate(s: string, max: number): string {
  if (s.length <= max) return s
  return s.slice(0, max) + `\n\n... [${s.length - max} more characters]`
}
