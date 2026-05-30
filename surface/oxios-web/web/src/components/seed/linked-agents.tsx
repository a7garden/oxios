import { Bot, Clock } from 'lucide-react'
import { useNavigate } from '@tanstack/react-router'
import { useTranslation } from 'react-i18next'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { EmptyState } from '@/components/shared/empty-state'
import { LoadingCards } from '@/components/shared/loading'
import { StatusIndicator } from '@/components/shared/status-indicator'
import { useSeedAgents } from '@/hooks/use-agent-trace'

export function LinkedAgents({ seedId }: { seedId: string }) {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const { data, isLoading } = useSeedAgents(seedId)

  if (isLoading) return <LoadingCards count={2} />

  const agents = data?.agents ?? []
  if (!agents.length) {
    return (
      <EmptyState
        icon={<Bot className="h-10 w-10" />}
        title={t('seeds.noLinkedAgents')}
      />
    )
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Bot className="h-4 w-4" /> {t('seeds.linkedAgents')}
        </CardTitle>
      </CardHeader>
      <CardContent>
        <div className="space-y-2">
          {agents.map((a) => (
            <div
              key={a.id}
              className="flex cursor-pointer items-center justify-between rounded-lg border p-3 hover:bg-muted/50"
              onClick={() =>
                navigate({ to: '/agents/$agentId', params: { agentId: a.id } })
              }
              onKeyDown={(e) => {
                if (e.key === 'Enter' || e.key === ' ') {
                  navigate({ to: '/agents/$agentId', params: { agentId: a.id } })
                }
              }}
              role="button"
              tabIndex={0}
            >
              <div className="flex items-center gap-2">
                <Bot className="h-4 w-4 text-muted-foreground" />
                <span className="text-sm font-medium">{a.name}</span>
                <StatusIndicator status={a.status?.toLowerCase() ?? 'unknown'} />
              </div>
              <div className="flex items-center gap-2 text-xs text-muted-foreground">
                <Clock className="h-3 w-3" />
                {a.created_at ? new Date(a.created_at).toLocaleString() : ''}
              </div>
            </div>
          ))}
        </div>
      </CardContent>
    </Card>
  )
}
