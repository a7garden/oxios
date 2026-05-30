import { useTranslation } from 'react-i18next'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent } from '@/components/ui/card'
import type { A2AAgentCard } from '@/types/a2a'

interface Props {
  agents: A2AAgentCard[]
}

const STATUS_COLOR: Record<string, string> = {
  active: 'bg-emerald-500',
  idle: 'bg-amber-500',
  stopped: 'bg-red-500',
  starting: 'bg-blue-500',
}

export function AgentCardList({ agents }: Props) {
  const { t } = useTranslation()

  if (agents.length === 0) {
    return (
      <div className="flex items-center justify-center h-48 text-muted-foreground">
        {t('a2a.noAgents')}
      </div>
    )
  }

  return (
    <div className="grid gap-4 md:grid-cols-2">
      {agents.map((agent) => (
        <Card key={agent.agent_id}>
          <CardContent className="p-5 space-y-3">
            <div className="flex items-start justify-between">
              <div>
                <h3 className="font-semibold">{agent.name}</h3>
                <p className="text-sm text-muted-foreground">{agent.description}</p>
              </div>
              <div className="flex items-center gap-1.5">
                <span className={`h-2.5 w-2.5 rounded-full ${STATUS_COLOR[agent.status] ?? 'bg-gray-400'}`} />
                <span className="text-xs capitalize">{agent.status}</span>
              </div>
            </div>

            {agent.capabilities.length > 0 && (
              <div className="flex flex-wrap gap-1">
                {agent.capabilities.map((cap) => (
                  <Badge key={cap} variant="outline" className="text-xs">{cap}</Badge>
                ))}
              </div>
            )}

            {agent.skills.length > 0 && (
              <div className="flex flex-wrap gap-1">
                {agent.skills.map((skill) => (
                  <Badge key={skill} variant="secondary" className="text-xs">{skill}</Badge>
                ))}
              </div>
            )}

            <p className="text-xs text-muted-foreground">
              {t('a2a.endpoint')}: {agent.endpoint}
            </p>
          </CardContent>
        </Card>
      ))}
    </div>
  )
}
