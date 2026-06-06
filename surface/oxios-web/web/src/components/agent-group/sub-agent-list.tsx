import { CheckCircle2, Clock, Play, XCircle } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent } from '@/components/ui/card'
import type { GroupAgent } from '@/types/agent-group'

interface Props {
  agents: GroupAgent[]
}

const STATUS_META: Record<
  string,
  { icon: React.ReactNode; variant: 'success' | 'warning' | 'destructive' | 'secondary' }
> = {
  Pending: { icon: <Clock className="h-3 w-3" />, variant: 'secondary' },
  Running: { icon: <Play className="h-3 w-3" />, variant: 'success' },
  Completed: { icon: <CheckCircle2 className="h-3 w-3" />, variant: 'success' },
  Failed: { icon: <XCircle className="h-3 w-3" />, variant: 'destructive' },
}

export function SubAgentList({ agents }: Props) {
  return (
    <div className="space-y-3">
      {agents.map((agent) => {
        const meta = STATUS_META[agent.status] ?? STATUS_META.Pending!
        return (
          <Card key={agent.id}>
            <CardContent className="p-4 space-y-2">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <Badge variant={meta.variant} className="gap-1">
                    {meta.icon} {agent.status}
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
