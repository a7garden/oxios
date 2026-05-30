import { useTranslation } from 'react-i18next'
import type { AgentDetail } from '@/types/agent'

interface BudgetBarProps {
  agent: AgentDetail
}

export function AgentBudgetBar({ agent }: BudgetBarProps) {
  const { t } = useTranslation()

  // AgentDetail exposes tokens_used and cost_usd directly (no limits)
  const tokensUsed = agent.tokens_used ?? 0
  const costUsed = agent.cost_usd ?? 0

  return (
    <div className="flex gap-4 text-xs text-muted-foreground">
      <span>
        {t('agents.tokens')}: {tokensUsed.toLocaleString()}
      </span>
      <span>
        {t('agents.cost')}: ${costUsed.toFixed(2)}
      </span>
      {agent.steps_total != null && (
        <span>
          {t('agents.stepsCompleted')}: {agent.steps_completed}/{agent.steps_total}
        </span>
      )}
      {agent.steps_total == null && (
        <span>
          {t('agents.stepsCompleted')}: {agent.steps_completed}
        </span>
      )}
    </div>
  )
}
