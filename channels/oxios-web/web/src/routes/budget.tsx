import { useQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { RefreshCw, Wallet } from 'lucide-react'
import { EmptyState } from '@/components/shared/empty-state'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { api } from '@/lib/api-client'
import type { Budget } from '@/types'

export const Route = createFileRoute('/budget')({ component: BudgetPage })

function BudgetPage() {
  const {
    data: budgets,
    isLoading,
    isError,
    refetch,
    isFetching,
  } = useQuery({
    queryKey: ['budgets'],
    queryFn: async (): Promise<Budget[]> => {
      const res = await api.get<{ items: Budget[] }>('/api/budget')
      return res.items ?? []
    },
    refetchInterval: 10000,
  })

  if (isLoading) return <LoadingCards count={4} />
  if (isError) return <ErrorState onRetry={() => refetch()} />

  const items = budgets ?? []

  const totalTokens = items.reduce((acc, b) => acc + (b.tokens_used ?? 0), 0)
  const totalCost = items.reduce((acc, b) => acc + (b.cost_used ?? 0), 0)

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Budget</h1>
          <p className="text-muted-foreground">Token and cost budget tracking</p>
        </div>
        <button
          type="button"
          onClick={() => refetch()}
          aria-label="Refresh"
          disabled={isFetching}
          className="rounded-md p-2 hover:bg-muted"
        >
          <RefreshCw className={`h-4 w-4 ${isFetching ? 'animate-spin' : ''}`} />
        </button>
      </div>

      {/* Summary */}
      <div className="grid gap-4 md:grid-cols-2">
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm text-muted-foreground">Total Tokens Used</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">{totalTokens.toLocaleString()}</div>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm text-muted-foreground">Total Cost</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">${totalCost.toFixed(4)}</div>
          </CardContent>
        </Card>
      </div>

      {/* Per-Agent Budget */}
      {items.length === 0 ? (
        <EmptyState
          icon={<Wallet className="h-10 w-10" />}
          title="No budget data"
          description="Budget information will appear as agents consume tokens."
        />
      ) : (
        <div className="space-y-3">
          {items.map((budget) => {
            const tokenPercent = budget.tokens_limit
              ? Math.min(100, ((budget.tokens_used ?? 0) / budget.tokens_limit) * 100)
              : 0
            const costPercent = budget.cost_limit
              ? Math.min(100, ((budget.cost_used ?? 0) / budget.cost_limit) * 100)
              : 0

            return (
              <Card key={budget.agent_id}>
                <CardHeader className="pb-2">
                  <CardTitle className="text-sm flex items-center gap-2">
                    <Wallet className="h-4 w-4" />
                    <span className="font-mono">{budget.agent_id.slice(0, 12)}...</span>
                  </CardTitle>
                </CardHeader>
                <CardContent className="space-y-3">
                  <div>
                    <div className="flex justify-between text-sm mb-1">
                      <span>Tokens: {(budget.tokens_used ?? 0).toLocaleString()}</span>
                      <span className="text-muted-foreground">
                        / {budget.tokens_limit?.toLocaleString() ?? '∞'}
                      </span>
                    </div>
                    <div className="h-2 rounded-full bg-muted overflow-hidden">
                      <div
                        className="h-full rounded-full bg-primary transition-all"
                        style={{ width: `${tokenPercent}%` }}
                      />
                    </div>
                  </div>
                  <div>
                    <div className="flex justify-between text-sm mb-1">
                      <span>Cost: ${(budget.cost_used ?? 0).toFixed(4)}</span>
                      <span className="text-muted-foreground">
                        / ${budget.cost_limit?.toFixed(2) ?? '∞'}
                      </span>
                    </div>
                    <div className="h-2 rounded-full bg-muted overflow-hidden">
                      <div
                        className="h-full rounded-full bg-amber-500 transition-all"
                        style={{ width: `${costPercent}%` }}
                      />
                    </div>
                  </div>
                </CardContent>
              </Card>
            )
          })}
        </div>
      )}
    </div>
  )
}
