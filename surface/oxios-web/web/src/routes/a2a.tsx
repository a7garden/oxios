import { createFileRoute } from '@tanstack/react-router'
import { Network } from 'lucide-react'
import { useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { RefreshButton } from '@/components/shared/refresh-button'
import { cn } from '@/lib/utils'
import { useA2AAgents, useA2AMessages, useA2ATopology } from '@/hooks/use-a2a'
import { InteractiveTopology } from '@/components/a2a/interactive-topology'
import { MessageLog } from '@/components/a2a/message-log'
import { AgentCardList } from '@/components/a2a/agent-card-list'
import { AgentInspector } from '@/components/a2a/agent-inspector'
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

  const refetchAll = () => {
    agentsQ.refetch()
    messagesQ.refetch()
    topologyQ.refetch()
  }

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
    return (messagesQ.data ?? [])
      .filter((m) => m.from_agent === selectedNodeId || m.to_agent === selectedNodeId)
      .slice(0, 5)
  }, [messagesQ.data, selectedNodeId])

  const handleNodeSelect = (id: string) => {
    setSelectedNodeId(id)
  }

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
          nodes={topologyQ.data?.nodes ?? []}
          edges={topologyQ.data?.edges ?? []}
          isLoading={topologyQ.isLoading}
          isError={topologyQ.isError}
          onRetry={() => topologyQ.refetch()}
          onNodeSelect={handleNodeSelect}
          selectedNodeId={selectedNodeId}
        />
      )}
      {tab === 'messages' && <MessageLog messages={messagesQ.data ?? []} />}
      {tab === 'agents' && <AgentCardList agents={agentsQ.data ?? []} />}

      <AgentInspector
        node={selectedNode}
        open={selectedNode != null}
        onClose={() => setSelectedNodeId(null)}
        agentCard={selectedAgentCard}
        recentMessages={selectedMessages}
        isMessagesLoading={messagesQ.isLoading}
        onViewTrace={(id) => {
          // Future: route to /agents/{id}/trace
          // For now, log to the console for traceability.
          // eslint-disable-next-line no-console
          console.info('[a2a] view trace', id)
        }}
        onStopAgent={(id) => {
          // Future: POST /api/agents/{id}/stop
          // eslint-disable-next-line no-console
          console.info('[a2a] stop agent', id)
        }}
      />
    </div>
  )
}
