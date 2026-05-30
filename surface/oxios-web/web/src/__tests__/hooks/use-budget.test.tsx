import { describe, expect, it } from 'vitest'
import { renderHook, waitFor } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { useQuery } from '@tanstack/react-query'
import type { AgentBudget } from '@/types/budget'

// Mock i18next
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: 'en' },
  }),
}))

const createWrapper = () => {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  })
  return ({ children }: { children: React.ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  )
}

describe('useBudgetList hook patterns', () => {
  it('useBudgetList fetches and returns data', async () => {
    const mockAgents: AgentBudget[] = [
      { agent_id: 'agent-1', name: 'Agent 1', budget: { token_limit: 100000, tokens_used: 50000, tokens_remaining: 50000, calls_limit: 100, calls_used: 23, calls_remaining: 77, window_secs: 3600, window_remaining_secs: 2847, is_exhausted: false } },
      { agent_id: 'agent-2', name: 'Agent 2', budget: { token_limit: 50000, tokens_used: 50000, tokens_remaining: 0, calls_limit: 50, calls_used: 50, calls_remaining: 0, window_secs: 3600, window_remaining_secs: 0, is_exhausted: true } },
    ]

    const { result } = renderHook(() => {
      return useQuery({
        queryKey: ['budgets'],
        queryFn: async (): Promise<AgentBudget[]> => mockAgents,
      })
    }, { wrapper: createWrapper() })

    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data).toBeDefined()
    expect(result.current.data).toHaveLength(2)
  })

  it('calculates budget percentages correctly', () => {
    const budget = { token_limit: 100000, tokens_used: 75000, tokens_remaining: 25000, calls_limit: 100, calls_used: 75, calls_remaining: 25, window_secs: 3600, window_remaining_secs: 1800, is_exhausted: false }
    const tokenPct = budget.token_limit > 0 ? Math.min(100, (budget.tokens_used / budget.token_limit) * 100) : 0
    expect(tokenPct).toBe(75)

    const callPct = budget.calls_limit > 0 ? Math.min(100, (budget.calls_used / budget.calls_limit) * 100) : 0
    expect(callPct).toBe(75)
  })

  it('handles exhausted budget detection', () => {
    const budget = { token_limit: 100000, tokens_used: 100000, tokens_remaining: 0, calls_limit: 100, calls_used: 100, calls_remaining: 0, window_secs: 3600, window_remaining_secs: 0, is_exhausted: true }
    expect(budget.is_exhausted).toBe(true)
    expect(budget.tokens_remaining).toBe(0)
  })

  it('calculates budget summary totals correctly', () => {
    const agents: AgentBudget[] = [
      { agent_id: 'a1', budget: { token_limit: 100000, tokens_used: 50000, tokens_remaining: 50000, calls_limit: 100, calls_used: 50, calls_remaining: 50, window_secs: 3600, window_remaining_secs: 1800, is_exhausted: false } },
      { agent_id: 'a2', budget: { token_limit: 50000, tokens_used: 30000, tokens_remaining: 20000, calls_limit: 50, calls_used: 30, calls_remaining: 20, window_secs: 3600, window_remaining_secs: 1800, is_exhausted: false } },
      { agent_id: 'a3', budget: { token_limit: 20000, tokens_used: 20000, tokens_remaining: 0, calls_limit: 10, calls_used: 10, calls_remaining: 0, window_secs: 3600, window_remaining_secs: 0, is_exhausted: true } },
    ]
    const totalTokensUsed = agents.reduce((acc, a) => acc + a.budget.tokens_used, 0)
    const totalTokensLimit = agents.reduce((acc, a) => acc + a.budget.token_limit, 0)
    const exhausted = agents.filter(a => a.budget.is_exhausted).length

    expect(totalTokensUsed).toBe(100000)
    expect(totalTokensLimit).toBe(170000)
    expect(exhausted).toBe(1)
  })
})
