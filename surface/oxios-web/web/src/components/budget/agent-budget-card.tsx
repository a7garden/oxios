import { useTranslation } from 'react-i18next'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import type { AgentBudget } from '@/types/budget'

interface Props {
  agent: AgentBudget
  onEdit: (agent: AgentBudget) => void
  onReset: (agentId: string) => void
  onRemove: (agentId: string) => void
  isResetting: boolean
  isRemoving: boolean
}

function formatRemaining(secs: number): string {
  if (secs === 0) return '0s'
  const m = Math.floor(secs / 60)
  const s = secs % 60
  if (m >= 60) {
    const h = Math.floor(m / 60)
    const rm = m % 60
    return `${h}h ${rm}m`
  }
  return m > 0 ? `${m}m ${s}s` : `${s}s`
}

export function AgentBudgetCard({
  agent,
  onEdit,
  onReset,
  onRemove,
  isResetting,
  isRemoving,
}: Props) {
  const { t } = useTranslation()
  const b = agent.budget
  const name = agent.name || `${agent.agent_id.slice(0, 12)}...`

  const tokenPct = b.token_limit > 0 ? Math.min(100, (b.tokens_used / b.token_limit) * 100) : 0
  const callPct = b.calls_limit > 0 ? Math.min(100, (b.calls_used / b.calls_limit) * 100) : 0

  return (
    <Card className={b.is_exhausted ? 'border-error/50' : ''}>
      <CardHeader className="pb-2">
        <CardTitle className="text-sm flex items-center gap-2">
          <span
            className={`h-2.5 w-2.5 rounded-full ${b.is_exhausted ? 'bg-error' : 'bg-success'}`}
          />
          <span className="font-mono">{name}</span>
          {b.is_exhausted && (
            <Badge variant="destructive" className="text-xs">
              {t('budget.exhausted')}
            </Badge>
          )}
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-3">
        {/* Tokens */}
        <div>
          <div className="flex justify-between text-sm mb-1">
            <span>
              {t('budget.tokens')}: {b.tokens_used.toLocaleString()}
            </span>
            <span className="text-muted-foreground">/ {b.token_limit.toLocaleString()}</span>
          </div>
          <div className="h-2 rounded-full bg-muted overflow-hidden">
            <div
              className={`h-full rounded-full transition-all ${tokenPct >= 90 ? 'bg-error' : tokenPct >= 70 ? 'bg-warning' : 'bg-primary'}`}
              style={{ width: `${tokenPct}%` }}
            />
          </div>
        </div>

        {/* Calls */}
        <div>
          <div className="flex justify-between text-sm mb-1">
            <span>
              {t('budget.calls')}: {b.calls_used.toLocaleString()}
            </span>
            <span className="text-muted-foreground">/ {b.calls_limit.toLocaleString()}</span>
          </div>
          <div className="h-2 rounded-full bg-muted overflow-hidden">
            <div
              className={`h-full rounded-full transition-all ${callPct >= 90 ? 'bg-error' : callPct >= 70 ? 'bg-warning' : 'bg-info'}`}
              style={{ width: `${callPct}%` }}
            />
          </div>
        </div>

        {/* Window */}
        <p className="text-xs text-muted-foreground">
          {t('budget.windowRemaining')}:{' '}
          {b.is_exhausted ? t('budget.exhausted') : formatRemaining(b.window_remaining_secs)}
        </p>

        {/* Actions */}
        <div className="flex gap-2 pt-1">
          <Button size="sm" variant="outline" onClick={() => onEdit(agent)}>
            {t('budget.editLimit')}
          </Button>
          <Button
            size="sm"
            variant="outline"
            onClick={() => onReset(agent.agent_id)}
            disabled={isResetting}
          >
            {t('budget.resetWindow')}
          </Button>
          <Button
            size="sm"
            variant="ghost"
            className="text-destructive"
            onClick={() => onRemove(agent.agent_id)}
            disabled={isRemoving}
          >
            {t('budget.removeBudget')}
          </Button>
        </div>
      </CardContent>
    </Card>
  )
}
