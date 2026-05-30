import { Badge } from '@/components/ui/badge'
import { Card, CardContent } from '@/components/ui/card'
import type { GroupAgent } from '@/types/agent-group'

interface Props {
  agents: GroupAgent[]
}

const STATUS_META: Record<string, { emoji: string; variant: 'success' | 'warning' | 'destructive' | 'secondary' }> = {
  Pending: { emoji: '⏳', variant: 'secondary' },
  Running: { emoji: '🟢', variant: 'success' },
  Completed: { emoji: '✅', variant: 'success' },
  Failed: { emoji: '🔴', variant: 'destructive' },
}

export function SubAgentList({ agents }: Props) {
  return (
    <div className="space-y-3">
      {agents.map((agent) => {
        const meta = STATUS_META[agent.status] ?? STATUS_META['Pending']!
        return (
          <Card key={agent.id}>
            <CardContent className="p-4 space-y-2">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <Badge variant={meta.variant} className="gap-1">
                    <span>{meta.emoji}</span> {agent.status}
                  </Badge>
                  <span className="font-mono text-sm">{agent.id.slice(0, 8)}...</span>
                </div>
                <span className="text-xs text-muted-foreground">Gen {agent.seed.generation}</span>
              </div>
              <p className="text-sm">{agent.seed.goal}</p>
              {agent.result && (
                <p className="text-xs text-muted-foreground bg-muted/50 rounded px-2 py-1 line-clamp-2">
                  {agent.result}
                </p>
              )}
            </CardContent>
          </Card>
        )
      })}
    </div>
  )
}