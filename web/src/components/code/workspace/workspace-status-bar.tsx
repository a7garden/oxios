import { GitBranch, Circle, Loader2 } from 'lucide-react'
import { useCodeSessionStore } from '@/stores/code/code-session'

export function WorkspaceStatusBar() {
  const { session, gitBranch, pendingChanges, isAgentRunning, agentPhase, tabs } =
    useCodeSessionStore()

  const dirtyCount = tabs.filter((t) => t.isDirty).length

  return (
    <div className="flex h-7 items-center gap-3 border-t bg-surface-sunken px-3 text-xs text-muted-foreground">
      {gitBranch && (
        <span className="flex items-center gap-1">
          <GitBranch className="h-3 w-3" />
          {gitBranch}
        </span>
      )}

      {dirtyCount > 0 && (
        <span className="flex items-center gap-1 text-warning">
          <Circle className="h-2 w-2 fill-current" />
          {dirtyCount} unsaved
        </span>
      )}

      {pendingChanges.length > 0 && (
        <span>
          {pendingChanges.length} pending change{pendingChanges.length !== 1 ? 's' : ''}
        </span>
      )}

      <div className="flex-1" />

      {isAgentRunning ? (
        <span className="flex items-center gap-1 text-info">
          <Loader2 className="h-3 w-3 animate-spin" />
          {agentPhase ?? 'Working...'}
        </span>
      ) : (
        <span className="flex items-center gap-1">
          <Circle className="h-2 w-2 fill-success text-success" />
          Ready
        </span>
      )}

      {session?.model && <span>{session.model}</span>}
    </div>
  )
}
