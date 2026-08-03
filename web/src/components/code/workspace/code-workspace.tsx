import { useEffect } from 'react'
 import { Group, Panel, Separator } from 'react-resizable-panels'
import { useCodeLayoutStore } from '@/stores/code/code-session'
import { FileExplorer } from '@/components/code/explorer/file-explorer'
import { AgentPanel } from '@/components/code/agent/agent-panel'
import { CodeEditor } from '@/components/code/editor/code-editor'
import { TerminalPanel } from '@/components/code/terminal/terminal-panel'
import { ProjectCanvas } from '@/components/code/canvas/project-canvas'
import { WorkspaceHeader } from './workspace-header'
import { WorkspaceStatusBar } from './workspace-status-bar'

/**
 * Horizontal resize handle (between left/right panels).
 * Thin line that brightens on hover/drag, with an extended invisible
 * hit area for easier grabbing.
 */
function HSeparator() {
  return (
    <Separator className="relative w-px shrink-0 bg-border transition-colors data-[separator=hover]:bg-primary/50 data-[separator=drag]:bg-primary">
      <div className="absolute inset-y-0 -left-[3px] -right-[3px] z-10" />
    </Separator>
  )
}

/**
 * Vertical resize handle (between top/bottom panels).
 */
function VSeparator() {
  return (
    <Separator className="relative h-px shrink-0 bg-border transition-colors data-[separator=hover]:bg-primary/50 data-[separator=drag]:bg-primary">
      <div className="absolute inset-x-0 -top-[3px] -bottom-[3px] z-10" />
    </Separator>
  )
}

/**
 * Full-screen IDE workspace — three-panel resizable layout.
 *
 * Left:   File Explorer (collapsible)
 * Center: Editor/Canvas (top) + Terminal (bottom, collapsible)
 * Right:  Agent Panel (collapsible)
 *
 * Panel sizes are persisted per-browser via the `id` prop + Group
 * layout storage. Toggle visibility lives in useCodeLayoutStore.
 */
export function CodeWorkspace() {
  const { showExplorer, showAgent, showTerminal, showCanvas } = useCodeLayoutStore()
  const toggleExplorer = useCodeLayoutStore((s) => s.toggleExplorer)
  const toggleAgent = useCodeLayoutStore((s) => s.toggleAgent)
  const toggleTerminal = useCodeLayoutStore((s) => s.toggleTerminal)

  // IDE keyboard shortcuts: ⌘B (explorer), ⌘J (terminal), ⌘⇧A (agent).
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey
      if (!mod) return
      if (e.key === 'b' && !e.shiftKey) {
        e.preventDefault()
        toggleExplorer()
      } else if (e.key === 'j' && !e.shiftKey) {
        e.preventDefault()
        toggleTerminal()
      } else if (e.key === 'a' && e.shiftKey) {
        e.preventDefault()
        toggleAgent()
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [toggleExplorer, toggleTerminal, toggleAgent])

  return (
    <div className="flex h-full flex-col overflow-hidden bg-surface">
      <WorkspaceHeader />

      <div className="flex-1 overflow-hidden">
        <Group orientation="horizontal" style={{ height: '100%' }}>
          {showExplorer && (
            <Panel id="explorer" defaultSize="18" minSize="12" maxSize="35">
              <div className="h-full overflow-hidden">
                <FileExplorer />
              </div>
            </Panel>
          )}
          {showExplorer && <HSeparator />}

          {/* Center: editor + terminal split */}
          <Panel id="center" minSize="30">
            <Group orientation="vertical" style={{ height: '100%' }}>
              <Panel id="editor" defaultSize="70" minSize="20">
                <div className="h-full overflow-hidden">
                  {showCanvas ? <ProjectCanvas /> : <CodeEditor />}
                </div>
              </Panel>
              {showTerminal && <VSeparator />}
              {showTerminal && (
                <Panel id="terminal" defaultSize="30" minSize="10" maxSize="80">
                  <div className="h-full overflow-hidden">
                    <TerminalPanel />
                  </div>
                </Panel>
              )}
            </Group>
          </Panel>

          {showAgent && <HSeparator />}
          {showAgent && (
            <Panel id="agent" defaultSize="28" minSize="18" maxSize="50">
              <div className="h-full overflow-hidden">
                <AgentPanel />
              </div>
            </Panel>
          )}
        </Group>
      </div>

      <WorkspaceStatusBar />
    </div>
  )
}
