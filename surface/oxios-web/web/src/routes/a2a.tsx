import { createFileRoute } from '@tanstack/react-router'
import { Network } from 'lucide-react'
import { useCallback, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { AgentCardList } from '@/components/a2a/agent-card-list'
import { AgentInspector } from '@/components/a2a/agent-inspector'
import { InteractiveTopology } from '@/components/a2a/interactive-topology'
import { MessageLog } from '@/components/a2a/message-log'
import { RefreshButton } from '@/components/shared/refresh-button'
import { toast } from 'sonner'
import { useA2AAgents, useA2AMessages, useA2ATopology } from '@/hooks/use-a2a'
import { cn } from '@/lib/utils'
import type { A2AMessage } from '@/types/a2a'

export const Route = createFileRoute('/a2a')({ component: A2APage })

type Tab = 'topology' | 'messages' | 'agents'

function A2APage() {
  const { t } = useTranslation()
  const [tab, setTab] = useState<Tab>('topology')
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null)

  const agentsQ = useA2AAgents()
  const messagesQ = useA2AMessages()
  const topologyQ = useA2ATopology()

  const isFetching = agentsQ.isFetching || messagesQ.isFetching || topologyQ.isFetching

  const refetchAll = useCallback(() => {
    agentsQ.refetch()
    messagesQ.refetch()
    topologyQ.refetch()
  }, [agentsQ, messagesQ, topologyQ])

  const tabs: { key: Tab; labelKey: string }[] = [
    { key: 'topology', labelKey: 'a2a.topology' },
    { key: 'messages', labelKey: 'a2a.messages' },
    { key: 'agents', labelKey: 'a2a.agents' },
  ]

  // Find the selected node + matching agent card.
  const selectedNode = useMemo(
    () => topologyQ.data?.nodes.find((n) => n.id === selectedNodeId) ?? null,
    [topologyQ.data?.nodes, selectedNodeId],
  )

  const selectedAgentCard = useMemo(() => {
    if (!selectedNodeId) return null
    return agentsQ.data?.find((a) => a.name === selectedNodeId) ?? null
  }, [agentsQ.data, selectedNodeId])

  // Most recent 5 messages involving the selected agent.
  const selectedMessages: A2AMessage[] = useMemo(() => {
    if (!selectedNodeId) return []
    return (Array.isArray(messagesQ.data) ? messagesQ.data : [])
      .filter((m) => m.from_agent === selectedNodeId || m.to_agent === selectedNodeId)
      .slice(0, 5)
  }, [messagesQ.data, selectedNodeId])

  const handleNodeSelect = useCallback((id: string) => {
    setSelectedNodeId(id)
  }, [])

  const handleViewTrace = useCallback(
    (id: string) => {
      // Trace view is not yet implemented — surface the gap with a toast
      // rather than a silent console.info (which made the destructive
      // [Stop agent] button look broken).
      toast(t('a2a.traceNotImplemented'))
      // `id` is captured for the future router hook-up:
      //   navigate({ to: '/agents/$id/trace', params: { id } })
      void id
    },
    [toast, t],
  )

  const handleStopAgent = useCallback(
    (id: string) => {
      // Stop-agent endpoint is not yet implemented. Honest UX: tell the
      // user via toast instead of logging to the console.
      toast.error(t('a2a.stopNotImplemented'))
      // `id` is captured for the future mutation hook-up:
      //   api.stopAgent(id)
      void id
    },
    [toast, t],
  )

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold flex items-center gap-2">
            <Network className="h-6 w-6" /> {t('a2a.title')}
          </h1>
          <p className="text-muted-foreground">{t('a2a.subtitle')}</p>
        </div>
        <RefreshButton onClick={refetchAll} isFetching={isFetching} />
      </div>

      {/* Tab switcher */}
      <div className="inline-flex h-9 items-center rounded-lg bg-muted p-1 text-muted-foreground gap-0.5">
        {tabs.map((tb) => (
          <button
            type="button"
            key={tb.key}
            onClick={() => setTab(tb.key)}
            className={cn(
              'inline-flex items-center justify-center whitespace-nowrap rounded-md px-3 py-1 text-sm font-medium transition-all',
              tab === tb.key ? 'bg-background text-foreground shadow' : 'hover:bg-background/50',
            )}
          >
            {t(tb.labelKey)}
          </button>
        ))}
      </div>

      {/* Content */}
      {tab === 'topology' && (
        <InteractiveTopology
          nodes={Array.isArray(topologyQ.data?.nodes) ? topologyQ.data.nodes : []}
          edges={Array.isArray(topologyQ.data?.edges) ? topologyQ.data.edges : []}
          isLoading={topologyQ.isLoading}
          isError={topologyQ.isError}
          onRetry={() => topologyQ.refetch()}
          onNodeSelect={handleNodeSelect}
          selectedNodeId={selectedNodeId}
        />
      )}
      {tab === 'messages' && (
        <MessageLog messages={Array.isArray(messagesQ.data) ? messagesQ.data : []} />
      )}
      {tab === 'agents' && (
        <AgentCardList agents={Array.isArray(agentsQ.data) ? agentsQ.data : []} />
      )}

      <AgentInspector
        node={selectedNode}
        open={selectedNode != null}
        onClose={() => setSelectedNodeId(null)}
        agentCard={selectedAgentCard}
        recentMessages={selectedMessages}
        isMessagesLoading={messagesQ.isLoading}
        onViewTrace={handleViewTrace}
        onStopAgent={handleStopAgent}
      />
    </div>
  )
}
