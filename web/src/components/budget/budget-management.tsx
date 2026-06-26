import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Card, CardContent } from '@/components/ui/card'
import { AgentBudgetCard } from '@/components/budget/agent-budget-card'
import { BudgetSummaryCard } from '@/components/budget/budget-summary'
import { SetBudgetDialog } from '@/components/budget/set-budget-dialog'
import { useBudgetList, useBudgetSet, useBudgetDelete, useBudgetReset } from '@/hooks/use-budget'
import type { AgentBudget } from '@/types/budget'
import { Gauge } from 'lucide-react'

/** Agent budget management section — token/call budgets with sliding windows.
 *
 * Wires the previously orphaned budget components (AgentBudgetCard,
 * SetBudgetDialog, BudgetSummaryCard) into the cost page. These are
 * rate-limiting budgets (tokens + calls per window), separate from the
 * dollar-based spend limit.
 */
export function BudgetManagement() {
  const { t } = useTranslation()
  const { data, isLoading } = useBudgetList()
  const setMutation = useBudgetSet()
  const deleteMutation = useBudgetDelete()
  const resetMutation = useBudgetReset()

  const [dialogOpen, setDialogOpen] = useState(false)
  const [editingAgent, setEditingAgent] = useState<AgentBudget | null>(null)

  const agents = data?.agents ?? []
  const summary = data?.summary

  const handleEdit = (agent: AgentBudget) => {
    setEditingAgent(agent)
    setDialogOpen(true)
  }

  return (
    <div className="space-y-4">
      {summary && <BudgetSummaryCard summary={summary} />}

      <div className="flex items-center justify-between">
        <h3 className="flex items-center gap-2 text-sm font-medium text-muted-foreground">
          <Gauge className="h-4 w-4" />
          {t('budget.agentBudgets')}
        </h3>
      </div>

      {isLoading ? (
        <p className="text-sm text-muted-foreground py-4">{t('common.loading')}</p>
      ) : agents.length === 0 ? (
        <Card>
          <CardContent className="py-8 text-center">
            <p className="text-sm text-muted-foreground">{t('budget.noBudgetDataDescription')}</p>
          </CardContent>
        </Card>
      ) : (
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {agents.map((agent) => (
            <AgentBudgetCard
              key={agent.agent_id}
              agent={agent}
              onEdit={handleEdit}
              onReset={(id) => resetMutation.mutate(id)}
              onRemove={(id) => deleteMutation.mutate(id)}
              isResetting={resetMutation.isPending}
              isRemoving={deleteMutation.isPending}
            />
          ))}
        </div>
      )}

      <SetBudgetDialog
        open={dialogOpen}
        onOpenChange={setDialogOpen}
        agent={editingAgent}
        onSubmit={(data) => setMutation.mutate(data)}
        isPending={setMutation.isPending}
      />
    </div>
  )
}
