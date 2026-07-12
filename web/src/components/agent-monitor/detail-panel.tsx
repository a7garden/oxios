/**
 * Merges lifecycle detail (cost, tokens, trace, logs) with A2A data
 * (capabilities, skills, inter-agent messages). Slides from the right;
 * clicking another node swaps content without closing.
 *
 * Trace/Logs tabs reuse the existing agent components. Messages tab
 * shows A2A messages filtered to this agent. Kill and View-Trace
 * actions are wired to real endpoints/navigation.
 */

import { useMutation, useQueryClient } from '@tanstack/react-query'
import { useNavigate } from '@tanstack/react-router'
import { ExternalLink, Skull, X } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { AgentBudgetBar } from '@/components/agent/agent-budget-bar'
import { AgentLogs } from '@/components/agent/agent-logs'
import { ExecutionTrace } from '@/components/agent/execution-trace'
import { statusBorder, statusDot } from '@/components/shared/status-palette'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { useA2AMessages } from '@/hooks/use-a2a'
import {
  useAgentDetail,
  useAgentLogs as useAgentLogsHook,
  useAgentTrace,
} from '@/hooks/use-agent-trace'
import { api } from '@/lib/api-client'
import { cn } from '@/lib/utils'
import type { MonitorNode } from '@/types/agent-monitor'

interface DetailPanelProps {
  /** The selected monitor node (instant display), or null when closed. */
  node: MonitorNode | null
  onClose: () => void
}

type Tab = 'trace' | 'logs' | 'messages'

function formatDuration(secs: number | null): string {
  if (secs == null) return '—'
  if (secs < 60) return `${Math.floor(secs)}s`
  const m = Math.floor(secs / 60)
  if (m < 60) return `${m}m ${Math.floor(secs % 60)}s`
  return `${Math.floor(m / 60)}h ${m % 60}m`
}

export function DetailPanel({ node, onClose }: DetailPanelProps) {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const [tab, setTab] = useState<Tab>('trace')

  const agentId = node?.agentId ?? null

  // Lifecycle data (fetched for full detail).
  const { data: detail } = useAgentDetail(agentId)
  const { data: trace, isLoading: traceLoading } = useAgentTrace(agentId)
  const { data: logs } = useAgentLogsHook(agentId)

  // A2A messages involving this agent (filtered by name).
  const messagesQ = useA2AMessages()
  const agentMessages = useMemo(() => {
    if (!node) return []
    return (messagesQ.data ?? [])
      .filter((m) => m.from_agent === node.name || m.to_agent === node.name)
      .slice(0, 20)
  }, [messagesQ.data, node])

  // Kill mutation.
  const killMutation = useMutation({
    mutationFn: () => api.post(`/api/agents/${agentId}/kill`),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['agents'] })
      onClose()
    },
  })

  // Close on Escape.
  useEffect(() => {
    if (!node) return
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [node, onClose])

  // Reset to trace tab when agent changes.
  useEffect(() => setTab('trace'), [agentId])

  const isOpen = node !== null
  const isRunning = node?.displayStatus === 'running'

  return (
    <>
      {/* Backdrop — only on mobile where the panel covers the whole screen.
         On desktop the panel is a side sheet, so the canvas stays usable. */}
      {isOpen && (
        <div
          className="fixed inset-0 z-40 bg-background/40 backdrop-blur-[1px] transition-opacity md:hidden"
          onClick={onClose}
          aria-hidden="true"
        />
      )}

      {/* Panel */}
      <aside
        className={cn(
          'fixed right-0 top-0 z-50 flex h-full w-full max-w-md flex-col border-l bg-card shadow-2xl',
          'transition-transform duration-300 ease-[var(--animate-in-easing)]',
          isOpen ? 'translate-x-0' : 'translate-x-full',
        )}
      >
        {node && (
          <>
            {/* Header */}
            <header className="flex items-start justify-between border-b p-4">
              <div className="min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <span
                    className={cn(
                      'h-2.5 w-2.5 shrink-0 rounded-full',
                      statusDot(node.displayStatus),
                    )}
                    aria-hidden="true"
                  />
                  <h2 className="truncate text-lg font-semibold" title={node.name}>
                    {node.name}
                  </h2>
                </div>
                <div className="mt-1 flex items-center gap-2">
                  <Badge
                    variant="outline"
                    className={cn('gap-1 text-2xs', statusBorder(node.displayStatus))}
                  >
                    {node.displayStatus}
                  </Badge>
                  <span className="font-mono text-2xs text-muted-foreground">
                    {node.agentId.slice(0, 8)}…
                  </span>
                </div>
              </div>
              <Button variant="ghost" size="icon" onClick={onClose} aria-label={t('common.close')}>
                <X className="h-4 w-4" />
              </Button>
            </header>

            {/* Scrollable content */}
            <div className="flex-1 overflow-y-auto p-4">
              {/* Metrics row */}
              <div className="grid grid-cols-3 gap-2 text-center">
                <div className="rounded-lg border bg-background p-2">
                  <div className="text-2xs uppercase tracking-wide text-muted-foreground">
                    {t('agents.cost')}
                  </div>
                  <div className="font-mono text-sm font-medium">
                    ${node.lifecycle.cost_usd.toFixed(node.lifecycle.cost_usd < 0.01 ? 4 : 2)}
                  </div>
                </div>
                <div className="rounded-lg border bg-background p-2">
                  <div className="text-2xs uppercase tracking-wide text-muted-foreground">
                    {t('agents.tokens')}
                  </div>
                  <div className="font-mono text-sm font-medium">
                    {node.lifecycle.tokens_used.toLocaleString()}
                  </div>
                </div>
                <div className="rounded-lg border bg-background p-2">
                  <div className="text-2xs uppercase tracking-wide text-muted-foreground">
                    {t('agents.duration')}
                  </div>
                  <div className="font-mono text-sm font-medium">
                    {isRunning && node.lifecycle.duration_secs == null
                      ? '…'
                      : formatDuration(node.lifecycle.duration_secs)}
                  </div>
                </div>
              </div>

              {/* Budget bar (when detail loaded) */}
              {detail && (
                <div className="mt-3 rounded-lg border bg-background p-3">
                  <AgentBudgetBar agent={detail} />
                  {node.lifecycle.model_id && (
                    <div className="mt-1 text-2xs text-muted-foreground">
                      <span className="font-medium">{t('agents.model')}:</span>{' '}
                      <span className="font-mono">{node.lifecycle.model_id}</span>
                    </div>
                  )}
                </div>
              )}

              {/* Error card */}
              {node.lifecycle.error && (
                <div className="mt-3 rounded-lg border border-destructive/50 bg-destructive/5 p-3">
                  <div className="flex items-center gap-2 text-sm font-medium text-destructive">
                    <Skull className="h-4 w-4" />
                    {t('agents.error')}
                  </div>
                  <p className="mt-1 break-words text-xs text-destructive/80">
                    {node.lifecycle.error}
                  </p>
                </div>
              )}

              {/* A2A capabilities + skills */}
              {node.a2a && (
                <div className="mt-3 space-y-2">
                  {node.a2a.capabilities.length > 0 && (
                    <div>
                      <div className="text-2xs font-semibold uppercase tracking-wide text-muted-foreground">
                        {t('agentMonitor.capabilities')}
                      </div>
                      <div className="mt-1 flex flex-wrap gap-1">
                        {node.a2a.capabilities.map((c) => (
                          <span
                            key={c}
                            className="rounded bg-info-muted px-1.5 py-0.5 text-2xs font-medium text-info"
                          >
                            {c}
                          </span>
                        ))}
                      </div>
                    </div>
                  )}
                  {node.a2a.skills.length > 0 && (
                    <div>
                      <div className="text-2xs font-semibold uppercase tracking-wide text-muted-foreground">
                        {t('agentMonitor.skills')}
                      </div>
                      <div className="mt-1 flex flex-wrap gap-1">
                        {node.a2a.skills.map((s) => (
                          <span
                            key={s}
                            className="rounded bg-muted px-1.5 py-0.5 text-2xs font-medium text-muted-foreground"
                          >
                            {s}
                          </span>
                        ))}
                      </div>
                    </div>
                  )}
                </div>
              )}

              {/* Tabs: Trace | Logs | Messages */}
              <Tabs value={tab} onValueChange={(v) => setTab(v as Tab)} className="mt-4">
                <TabsList className="w-full">
                  <TabsTrigger value="trace">{t('agents.trace')}</TabsTrigger>
                  <TabsTrigger value="logs">{t('agents.logs')}</TabsTrigger>
                  <TabsTrigger value="messages">
                    {t('agentMonitor.messages')}
                    {agentMessages.length > 0 && (
                      <Badge variant="secondary" className="ml-1 text-2xs">
                        {agentMessages.length}
                      </Badge>
                    )}
                  </TabsTrigger>
                </TabsList>

                <TabsContent value="trace" className="mt-3">
                  <ExecutionTrace trace={trace} isLoading={traceLoading} />
                </TabsContent>

                <TabsContent value="logs" className="mt-3">
                  <AgentLogs logs={logs} />
                </TabsContent>

                <TabsContent value="messages" className="mt-3">
                  {agentMessages.length === 0 ? (
                    <p className="py-8 text-center text-sm text-muted-foreground">
                      {t('agentMonitor.noMessages')}
                    </p>
                  ) : (
                    <div className="space-y-1.5">
                      {agentMessages.map((m) => (
                        <div
                          key={m.request_id}
                          className="rounded border bg-background p-2 text-xs"
                        >
                          <div className="flex items-center justify-between">
                            <span className="font-medium text-info">{m.message_type}</span>
                            <span className="text-2xs text-muted-foreground">
                              {m.timestamp ? new Date(m.timestamp).toLocaleTimeString() : ''}
                            </span>
                          </div>
                          <div className="mt-0.5 text-muted-foreground">
                            {m.from_agent} → {m.to_agent}
                          </div>
                          {m.payload_summary && (
                            <div className="mt-1 break-words text-2xs">{m.payload_summary}</div>
                          )}
                        </div>
                      ))}
                    </div>
                  )}
                </TabsContent>
              </Tabs>
            </div>

            {/* Footer: actions */}
            <footer className="flex items-center gap-2 border-t p-3">
              <Button
                variant="outline"
                size="sm"
                className="flex-1"
                onClick={() =>
                  navigate({ to: '/agents/$agentId', params: { agentId: node.agentId } })
                }
              >
                <ExternalLink className="mr-1.5 h-3.5 w-3.5" />
                {t('agentMonitor.viewFullDetail')}
              </Button>
              {isRunning && (
                <Button
                  variant="destructive"
                  size="sm"
                  onClick={() => killMutation.mutate()}
                  disabled={killMutation.isPending}
                >
                  <Skull className="mr-1.5 h-3.5 w-3.5" />
                  {t('agents.kill')}
                </Button>
              )}
            </footer>
          </>
        )}
      </aside>
    </>
  )
}
