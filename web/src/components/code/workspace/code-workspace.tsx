import { useCodeLayoutStore } from '@/stores/code/code-session'
import { FileExplorer } from '@/components/code/explorer/file-explorer'
import { AgentPanel } from '@/components/code/agent/agent-panel'
import { CodeEditor } from '@/components/code/editor/code-editor'
import { TerminalPanel } from '@/components/code/terminal/terminal-panel'
import { ProjectCanvas } from '@/components/code/canvas/project-canvas'
import { WorkspaceHeader } from './workspace-header'
import { WorkspaceStatusBar } from './workspace-status-bar'

/**
 * Full-screen IDE workspace — three-panel layout.
 *
 * Left: File Explorer
 * Center: Editor (top) + Terminal (bottom, collapsible)
 * Right: Agent Panel
 *
 * Panel sizes are controlled by CSS flex-basis from useCodeLayoutStore.
 */
export function CodeWorkspace() {
  const { showExplorer, showAgent, showTerminal, showCanvas } = useCodeLayoutStore()

  return (
    <div className="flex h-full flex-col overflow-hidden bg-surface">
      <WorkspaceHeader />

      <div className="flex flex-1 overflow-hidden">
        {showExplorer && (
          <div
            className="flex-shrink-0 border-r border-border overflow-hidden"
            style={{ width: '240px' }}
          >
            <FileExplorer />
          </div>
        )}

        <div className="flex flex-1 flex-col overflow-hidden">
          <div className="flex-1 overflow-hidden">
            {showCanvas ? <ProjectCanvas /> : <CodeEditor />}
          </div>
          {showTerminal && (
            <div
              className="flex-shrink-0 border-t border-border overflow-hidden"
              style={{ height: '240px' }}
            >
              <TerminalPanel />
            </div>
          )}
        </div>

        {showAgent && (
          <div
            className="flex-shrink-0 border-l border-border overflow-hidden"
            style={{ width: '380px' }}
          >
            <AgentPanel />
          </div>
        )}
      </div>

      <WorkspaceStatusBar />
    </div>
  )
}
