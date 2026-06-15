import { CheckCircle2, Clock, Play, XCircle } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent } from '@/components/ui/card'
import type { AgentGroup } from '@/types/agent-group'

interface Props {
  group: AgentGroup
  onClick: () => void
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

export function GroupCard({ group, onClick }: Props) {
  const { t } = useTranslation()

  // Derive status from agents
  const total = group.agents.length
  const completed = group.agents.filter((a) => a.status === 'Completed').length
  const failed = group.agents.filter((a) => a.status === 'Failed').length
  const running = group.agents.filter((a) => a.status === 'Running').length
  const pct = total > 0 ? Math.round((completed / total) * 100) : 0

  let status = 'Pending'
  if (failed > 0) status = 'Failed'
  else if (completed === total && total > 0) status = 'Completed'
  else if (running > 0 || completed > 0) status = 'Running'

  const meta = STATUS_META[status] ?? STATUS_META.Pending!

  return (
    <Card className="cursor-pointer select-none transition-shadow hover:shadow-md" onClick={onClick}>
      <CardContent className="p-5 space-y-3">
        <div className="flex items-start justify-between">
          <div>
            <h3 className="font-semibold">{group.id.slice(0, 8)}...</h3>
            {group.parent_seed_id && (
              <p className="text-sm text-muted-foreground">
                {t('agentGroups.parentSeed')}: {group.parent_seed_id.slice(0, 8)}...
              </p>
            )}
          </div>
          <Badge variant={meta.variant} className="gap-1">
            {meta.icon} {status}
          </Badge>
        </div>

        {/* Progress */}
        <div>
          <div className="flex justify-between text-sm mb-1">
            <span>{t('agentGroups.progress')}</span>
            <span className="text-muted-foreground">
              {pct}% ({completed}/{total})
            </span>
          </div>
          <div className="h-2 rounded-full bg-muted overflow-hidden">
            <div
              className={`h-full rounded-full transition-all ${status === 'Completed' ? 'bg-success' : status === 'Failed' ? 'bg-error' : 'bg-primary'}`}
              style={{ width: `${pct}%` }}
            />
          </div>
        </div>

        <p className="text-xs text-muted-foreground">
          {t('agentGroups.subAgents')}: {total}
          {failed > 0 && <span className="text-error ml-2">· {failed} failed</span>}
        </p>
      </CardContent>
    </Card>
  )
}
