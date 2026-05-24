import { useQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { Users } from 'lucide-react'
import { EmptyState } from '@/components/shared/empty-state'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { RefreshButton } from '@/components/shared/refresh-button'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { api } from '@/lib/api-client'
import type { AgentGroup } from '@/types'

export const Route = createFileRoute('/agent-groups')({ component: AgentGroupsPage })

function AgentGroupsPage() {
  const { data, isLoading, isError, refetch, isFetching } = useQuery({
    queryKey: ['agent-groups'],
    queryFn: () => api.get<AgentGroup[]>('/api/agent-groups'),
    refetchInterval: 30000,
  })

  if (isLoading) return <LoadingCards count={4} />
  if (isError) return <ErrorState onRetry={() => refetch()} />

  const groups = data ?? []

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Agent Groups</h1>
          <p className="text-muted-foreground">Multi-agent group management</p>
        </div>
        <RefreshButton onClick={() => refetch()} isFetching={isFetching} />
      </div>

      {groups.length === 0 ? (
        <EmptyState
          icon={<Users className="h-10 w-10" />}
          title="No agent groups"
          description="Groups are created when seeds split into multi-agent executions."
        />
      ) : (
        <div className="grid gap-4 md:grid-cols-2">
          {groups.map((group) => (
            <Card key={group.id}>
              <CardHeader className="pb-2">
                <CardTitle className="text-base flex items-center gap-2">
                  <Users className="h-4 w-4" /> {group.name}
                  {group.strategy && <Badge variant="outline">{group.strategy}</Badge>}
                </CardTitle>
                <p className="text-xs text-muted-foreground font-mono">{group.id}</p>
              </CardHeader>
              <CardContent>
                <p className="text-sm text-muted-foreground mb-2">{group.agents.length} agent(s)</p>
                <div className="flex gap-2 flex-wrap">
                  {group.agents.map((agentId) => (
                    <Badge key={agentId} variant="secondary" className="font-mono text-xs">
                      {agentId.slice(0, 8)}...
                    </Badge>
                  ))}
                </div>
              </CardContent>
            </Card>
          ))}
        </div>
      )}
    </div>
  )
}
