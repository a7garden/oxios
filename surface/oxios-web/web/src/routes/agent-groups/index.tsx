import { useNavigate } from '@tanstack/react-router'
import { createFileRoute } from '@tanstack/react-router'
import { Users } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { EmptyState } from '@/components/shared/empty-state'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { RefreshButton } from '@/components/shared/refresh-button'
import { useAgentGroups } from '@/hooks/use-agent-groups'
import { GroupCard } from '@/components/agent-group/group-card'

export const Route = createFileRoute('/agent-groups/')({ component: AgentGroupsPage })

function AgentGroupsPage() {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const { data: groups, isLoading, isError, refetch, isFetching } = useAgentGroups()

  if (isLoading) return <LoadingCards count={4} />
  if (isError) return <ErrorState onRetry={() => refetch()} />

  const items = groups ?? []

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold flex items-center gap-2">
            <Users className="h-6 w-6" /> {t('agentGroups.title')}
          </h1>
          <p className="text-muted-foreground">{t('agentGroups.subtitle')}</p>
        </div>
        <RefreshButton onClick={() => refetch()} isFetching={isFetching} />
      </div>

      {items.length === 0 ? (
        <EmptyState
          icon={<Users className="h-10 w-10" />}
          title={t('agentGroups.noGroups')}
          description={t('agentGroups.noGroupsDescription')}
        />
      ) : (
        <div className="grid gap-4">
          {items.map((group) => (
            <GroupCard
              key={group.id}
              group={group}
              onClick={() => navigate({ to: '/agent-groups/$groupId', params: { groupId: group.id } })}
            />
          ))}
        </div>
      )}
    </div>
  )
}
