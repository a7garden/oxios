import { useTranslation } from 'react-i18next'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent } from '@/components/ui/card'
import type { AgentGroup } from '@/types/agent-group'

interface Props {
  group: AgentGroup
  onClick: () => void
}

const STATUS_META: Record<string, { emoji: string; variant: 'success' | 'warning' | 'destructive' | 'secondary' }> = {
  Pending: { emoji: '⏳', variant: 'secondary' },
  Running: { emoji: '🟢', variant: 'success' },
  Completed: { emoji: '✅', variant: 'success' },
  Failed: { emoji: '🔴', variant: 'destructive' },
}

export function GroupCard({ group, onClick }: Props) {
  const { t } = useTranslation()

  // Derive status from agents
  const total = group.agents.length
  const completed = group.agents.filter(a => a.status === 'Completed').length
  const failed = group.agents.filter(a => a.status === 'Failed').length
  const running = group.agents.filter(a => a.status === 'Running').length
  const pct = total > 0 ? Math.round((completed / total) * 100) : 0

  let status = 'Pending'
  if (failed > 0) status = 'Failed'
  else if (completed === total && total > 0) status = 'Completed'
  else if (running > 0 || completed > 0) status = 'Running'

  const meta = STATUS_META[status] ?? STATUS_META['Pending']!

  return (
    <Card className="cursor-pointer transition-shadow hover:shadow-md" onClick={onClick}>
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
            <span>{meta.emoji}</span> {status}
          </Badge>
        </div>

        {/* Progress */}
        <div>
          <div className="flex justify-between text-sm mb-1">
            <span>{t('agentGroups.progress')}</span>
            <span className="text-muted-foreground">{pct}% ({completed}/{total})</span>
          </div>
          <div className="h-2 rounded-full bg-muted overflow-hidden">
            <div
              className={`h-full rounded-full transition-all ${status === 'Completed' ? 'bg-emerald-500' : status === 'Failed' ? 'bg-red-500' : 'bg-primary'}`}
              style={{ width: `${pct}%` }}
            />
          </div>
        </div>

        <p className="text-xs text-muted-foreground">
          {t('agentGroups.subAgents')}: {total}
          {failed > 0 && <span className="text-red-500 ml-2">· {failed} failed</span>}
        </p>
      </CardContent>
    </Card>
  )
}