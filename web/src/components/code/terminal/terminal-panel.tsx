/**
 * TerminalPanel — bottom-panel chrome for the code workspace.
 *
 * Owns the multi-tab terminal UI:
 *   - a top bar with one tab per open terminal and a "+" to spawn a new PTY,
 *   - a single active `TerminalView` for each PTY (hidden tabs are kept mounted
 *     so their scrollback & WS survive switching).
 *
 * Persistence is intentionally minimal — terminal tabs live in
 * `useCodeSessionStore.terminalIds`. The store is reset when the workspace
 * route unmounts (see code-workspace-route.tsx), which keeps the backend PTYs
 * in lock-step with their visible tabs.
 */
import { Plus, Terminal as TerminalIcon, X, Loader2 } from 'lucide-react'
import { useCallback, useEffect, useMemo, useState } from 'react'
import { Button } from '@/components/ui/button'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { codeApi } from '@/lib/code-api'
import { useCodeSessionStore, useCodeLayoutStore } from '@/stores/code/code-session'
import { cn } from '@/lib/utils'
import { TerminalView } from './terminal-view'

export function TerminalPanel() {
  const session = useCodeSessionStore((s) => s.session)
  const terminalIds = useCodeSessionStore((s) => s.terminalIds)
  const addTerminal = useCodeSessionStore((s) => s.addTerminal)
  const removeTerminal = useCodeSessionStore((s) => s.removeTerminal)
  const showTerminal = useCodeLayoutStore((s) => s.showTerminal)

  const [activeTid, setActiveTid] = useState<string | null>(null)
  const [creating, setCreating] = useState(false)
  const [createError, setCreateError] = useState<string | null>(null)

  // Make sure we always have a valid active tab.
  useEffect(() => {
    if (terminalIds.length === 0) {
      setActiveTid(null)
      return
    }
    if (!activeTid || !terminalIds.includes(activeTid)) {
      setActiveTid(terminalIds[0] ?? null)
    }
  }, [terminalIds, activeTid])

  const handleNew = useCallback(async () => {
    if (!session) return
    setCreating(true)
    setCreateError(null)
    try {
      const { terminal_id } = await codeApi.createTerminal(session.id)
      addTerminal(terminal_id)
      setActiveTid(terminal_id)
    } catch (e) {
      setCreateError(e instanceof Error ? e.message : 'Failed to create terminal')
    } finally {
      setCreating(false)
    }
  }, [session, addTerminal])

  const handleClose = useCallback(
    async (tid: string) => {
      removeTerminal(tid)
      // Backend cleanup happens in TerminalView's unmount; we don't await
      // here so the tab disappears immediately even if the daemon is slow.
    },
    [removeTerminal],
  )

  const tabs = useMemo(() => terminalIds, [terminalIds])

  if (!showTerminal) return null

  return (
    <div className="flex h-full flex-col bg-surface text-text">
      <div className="flex h-9 items-center gap-1 border-b bg-surface px-2">
        <TerminalIcon className="h-3.5 w-3.5 text-muted-foreground" />
        <span className="px-1 text-xs font-medium text-muted-foreground">Terminal</span>

        <div className="ml-2 flex h-full flex-1 items-center gap-1 overflow-x-auto">
          {tabs.length === 0 && (
            <span className="text-xs text-muted-foreground/70">No terminals — click + to start one</span>
          )}
          {tabs.map((tid) => {
            const isActive = tid === activeTid
            const label = tid.slice(0, 6)
            return (
              <div
                key={tid}
                role="tab"
                aria-selected={isActive}
                className={cn(
                  'group flex h-7 items-center gap-1.5 rounded-md border px-2 text-xs transition-colors',
                  isActive
                    ? 'border-line bg-surface-sunken text-text'
                    : 'border-transparent text-muted-foreground hover:bg-surface-sunken hover:text-text',
                )}
              >
                <button
                  type="button"
                  onClick={() => setActiveTid(tid)}
                  className="flex items-center gap-1.5"
                >
                  <TerminalIcon className="h-3 w-3" />
                  <span className="font-mono">{label}</span>
                </button>
                <button
                  type="button"
                  aria-label={`Close terminal ${label}`}
                  onClick={(e) => {
                    e.stopPropagation()
                    void handleClose(tid)
                  }}
                  className={cn(
                    'rounded p-0.5 text-muted-foreground hover:bg-line hover:text-text',
                    !isActive && 'opacity-0 group-hover:opacity-100',
                  )}
                >
                  <X className="h-3 w-3" />
                </button>
              </div>
            )
          })}
        </div>

        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              size="icon"
              variant="ghost"
              className="h-7 w-7"
              onClick={() => void handleNew()}
              disabled={creating || !session}
            >
              {creating ? (
                <Loader2 className="h-3.5 w-3.5 animate-spin" />
              ) : (
                <Plus className="h-3.5 w-3.5" />
              )}
            </Button>
          </TooltipTrigger>
          <TooltipContent>New terminal</TooltipContent>
        </Tooltip>
      </div>

      {createError && (
        <div className="border-b bg-destructive/10 px-3 py-1 text-xs text-destructive">
          {createError}
        </div>
      )}

      <div className="flex-1 overflow-hidden bg-surface-sunken">
        {tabs.length === 0 ? (
          <div className="flex h-full items-center justify-center text-xs text-muted-foreground">
            <div className="flex flex-col items-center gap-2">
              <TerminalIcon className="h-6 w-6 opacity-40" />
              <span>No terminals yet</span>
            </div>
          </div>
        ) : (
          tabs.map((tid) => (
            <TerminalView key={tid} terminalId={tid} active={tid === activeTid} />
          ))
        )}
      </div>
    </div>
  )
}
