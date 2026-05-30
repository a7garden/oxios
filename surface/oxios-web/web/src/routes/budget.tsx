import { useState } from 'react'
import { createFileRoute } from '@tanstack/react-router'
import { Plus, Wallet } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { EmptyState } from '@/components/shared/empty-state'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { RefreshButton } from '@/components/shared/refresh-button'
import { Button } from '@/components/ui/button'
import { useBudgetDelete, useBudgetList, useBudgetReset, useBudgetSet } from '@/hooks/use-budget'
import { useToast } from '@/components/ui/sonner'
import { BudgetSummaryCard } from '@/components/budget/budget-summary'
import { AgentBudgetCard } from '@/components/budget/agent-budget-card'
import { SetBudgetDialog } from '@/components/budget/set-budget-dialog'
import type { AgentBudget } from '@/types/budget'

export const Route = createFileRoute('/budget')({ component: BudgetPage })

function BudgetPage() {
  const { t } = useTranslation()
  const { toast } = useToast()
  const {
    data,
    isLoading,
    isError,
    refetch,
    isFetching,
  } = useBudgetList()

  const setMutation = useBudgetSet()
  const deleteMutation = useBudgetDelete()
  const resetMutation = useBudgetReset()

  const [dialogOpen, setDialogOpen] = useState(false)
  const [editingAgent, setEditingAgent] = useState<AgentBudget | null>(null)
  const [newAgentId, setNewAgentId] = useState('')

  if (isLoading) return <LoadingCards count={4} />
  if (isError) return <ErrorState onRetry={() => refetch()} />

  const agents = data?.agents ?? []
  const summary = data?.summary ?? { total_agents: 0, total_tokens_used: 0, total_tokens_limit: 0, exhausted_agents: 0 }

  const handleEdit = (agent: AgentBudget) => {
    setEditingAgent(agent)
    setNewAgentId('')
    setDialogOpen(true)
  }

  const handleNewBudget = () => {
    setEditingAgent(null)
    setNewAgentId('')
    setDialogOpen(true)
  }

  const handleSubmit = (params: { agentId: string; token_budget: number; calls_budget: number; window_secs: number }) => {
    setMutation.mutate(params, {
      onSuccess: () => toast(t('budget.setSuccess'), 'success'),
      onError: (e: unknown) => toast(e instanceof Error ? e.message : t('common.error'), 'destructive'),
    })
  }

  const handleReset = (agentId: string) => {
    resetMutation.mutate(agentId, {
      onSuccess: () => toast(t('budget.resetSuccess'), 'success'),
    })
  }

  const handleRemove = (agentId: string) => {
    deleteMutation.mutate(agentId, {
      onSuccess: () => toast(t('budget.removeSuccess'), 'success'),
    })
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">{t('budget.title')}</h1>
          <p className="text-muted-foreground">{t('budget.subtitle')}</p>
        </div>
        <div className="flex items-center gap-2">
          <Button size="sm" onClick={handleNewBudget} className="gap-1.5">
            <Plus className="h-4 w-4" /> {t('budget.setBudget')}
          </Button>
          <RefreshButton onClick={() => refetch()} isFetching={isFetching} />
        </div>
      </div>

      {/* Summary */}
      <BudgetSummaryCard summary={summary} />

      {/* Agent Budgets */}
      {agents.length === 0 ? (
        <EmptyState
          icon={<Wallet className="h-10 w-10" />}
          title={t('budget.noBudgetData')}
          description={t('budget.noBudgetDataDescription')}
        />
      ) : (
        <div className="space-y-3">
          {agents.map((agent) => (
            <AgentBudgetCard
              key={agent.agent_id}
              agent={agent}
              onEdit={handleEdit}
              onReset={handleReset}
              onRemove={handleRemove}
              isResetting={resetMutation.isPending}
              isRemoving={deleteMutation.isPending}
            />
          ))}
        </div>
      )}

      {/* Set Budget Dialog */}
      <SetBudgetDialog
        open={dialogOpen}
        onOpenChange={setDialogOpen}
        agent={editingAgent}
        agentId={newAgentId || undefined}
        onSubmit={handleSubmit}
        isPending={setMutation.isPending}
      />
    </div>
  )
}
