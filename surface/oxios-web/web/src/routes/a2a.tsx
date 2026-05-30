import { createFileRoute } from '@tanstack/react-router'
import { Network } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { RefreshButton } from '@/components/shared/refresh-button'
import { cn } from '@/lib/utils'
import { useA2AAgents, useA2AMessages, useA2ATopology } from '@/hooks/use-a2a'
import { TopologyGraph } from '@/components/a2a/topology-graph'
import { MessageLog } from '@/components/a2a/message-log'
import { AgentCardList } from '@/components/a2a/agent-card-list'

export const Route = createFileRoute('/a2a')({ component: A2APage })

type Tab = 'topology' | 'messages' | 'agents'

function A2APage() {
  const { t } = useTranslation()
  const [tab, setTab] = useState<Tab>('topology')

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
        <TopologyGraph nodes={topologyQ.data?.nodes ?? []} />
      )}
      {tab === 'messages' && (
        <MessageLog messages={messagesQ.data ?? []} />
      )}
      {tab === 'agents' && (
        <AgentCardList agents={agentsQ.data ?? []} />
      )}
    </div>
  )
}
