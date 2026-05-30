import { createFileRoute, Link } from '@tanstack/react-router'
import { ArrowLeft } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { GroupProgress } from '@/components/agent-group/group-progress'
import { SubAgentList } from '@/components/agent-group/sub-agent-list'
import { useAgentGroupDetail, useAgentGroupProgress } from '@/hooks/use-agent-groups'

export const Route = createFileRoute('/agent-groups/$groupId')({ component: AgentGroupDetailPage })

function AgentGroupDetailPage() {
  const { t } = useTranslation()
  const { groupId } = Route.useParams()
  const { data: group, isLoading: l1, isError: e1 } = useAgentGroupDetail(groupId)
  const { data: progress, isLoading: l2, isError: e2 } = useAgentGroupProgress(groupId)

  if (l1 || l2) return <LoadingCards count={4} />
  if (e1 || e2 || !group) return <ErrorState />

  const status = progress?.status ?? 'Unknown'
  const pct = progress?.completion_pct ?? 0

  return (
    <div className="space-y-6">
      {/* Back */}
      <Link to="/agent-groups" className="inline-flex items-center gap-1 text-sm text-muted-foreground hover:text-foreground">
        <ArrowLeft className="h-4 w-4" /> {t('agentGroups.backToGroups')}
      </Link>

      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold flex items-center gap-2">
            {t('agentGroups.group')} #{group.id.slice(0, 8)}
            <Badge variant={status === 'Completed' ? 'success' : status === 'Failed' ? 'destructive' : 'default'}>
              {status}
            </Badge>
          </h1>
          {group.parent_seed_id && (
            <p className="text-sm text-muted-foreground">
              {t('agentGroups.parentSeed')}: {group.parent_seed_id.slice(0, 8)}...
            </p>
          )}
        </div>
        <span className="text-lg font-bold">{Math.round(pct)}% ({progress?.completed ?? 0}/{group.agents.length})</span>
      </div>

      <GroupProgress pct={pct} />

      {/* Sub-agents */}
      <Card>
        <CardHeader>
          <CardTitle className="text-base">{t('agentGroups.subAgents')}</CardTitle>
        </CardHeader>
        <CardContent>
          <SubAgentList agents={group.agents} />
        </CardContent>
      </Card>

      {/* Combined results */}
      {progress?.combined_results && (
        <Card>
          <CardHeader>
            <CardTitle className="text-base">{t('agentGroups.combinedResults')}</CardTitle>
          </CardHeader>
          <CardContent>
            <pre className="text-sm whitespace-pre-wrap bg-muted/50 rounded p-3 max-h-96 overflow-y-auto">
              {progress.combined_results}
            </pre>
          </CardContent>
        </Card>
      )}
    </div>
  )
}
