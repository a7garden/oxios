import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { act, renderHook, waitFor } from '@testing-library/react'
import { HttpResponse, http } from 'msw'
import { afterEach, describe, expect, it } from 'vitest'
import { useBudgetDelete, useBudgetList, useBudgetReset, useBudgetSet } from '@/hooks/use-budget'
import type { AgentBudget, BudgetListResponse, SetBudgetRequest } from '@/types/budget'
import { server } from '../msw/server'

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: 'en' },
  }),
}))

function createWrapper() {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false, refetchInterval: false },
      mutations: { retry: false },
    },
  })
  return ({ children }: { children: React.ReactNode }) => (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  )
}

const baseAgents: AgentBudget[] = [
  {
    agent_id: 'agent-1',
    name: 'Agent 1',
    budget: {
      token_limit: 100_000,
      tokens_used: 50_000,
      tokens_remaining: 50_000,
      calls_limit: 100,
      calls_used: 23,
      calls_remaining: 77,
      window_secs: 3600,
      window_remaining_secs: 2847,
      is_exhausted: false,
    },
  },
  {
    agent_id: 'agent-2',
    name: 'Agent 2',
    budget: {
      token_limit: 50_000,
      tokens_used: 50_000,
      tokens_remaining: 0,
      calls_limit: 50,
      calls_used: 50,
      calls_remaining: 0,
      window_secs: 3600,
      window_remaining_secs: 0,
      is_exhausted: true,
    },
  },
]

const sampleResponse: BudgetListResponse = {
  agents: baseAgents,
  summary: {
    total_agents: baseAgents.length,
    total_tokens_used: 100_000,
    total_tokens_limit: 150_000,
    exhausted_agents: 1,
  },
}

describe('useBudgetList', () => {
  afterEach(() => {
    server.resetHandlers()
  })

  it('GETs /api/budget and exposes the full BudgetListResponse (agents + summary)', async () => {
    let hitUrl: string | null = null
    server.use(
      http.get('/api/budget', ({ request }) => {
        hitUrl = request.url
        return HttpResponse.json(sampleResponse)
      }),
    )

    const { result } = renderHook(() => useBudgetList(), { wrapper: createWrapper() })

    await waitFor(() => expect(result.current.isSuccess).toBe(true))

    // The exact endpoint the production hook hits.
    expect(hitUrl).not.toBeNull()
    expect(new URL(hitUrl!).pathname).toBe('/api/budget')

    // Real BudgetListResponse wiring — agents + summary both surface via the hook.
    const data = result.current.data
    expect(data).toBeDefined()
    expect(data!.agents).toHaveLength(2)
    expect(data!.agents[0]?.agent_id).toBe('agent-1')
    expect(data!.summary.exhausted_agents).toBe(1)
    expect(data!.summary.total_tokens_used).toBe(100_000)
  })
})

describe('useBudgetSet', () => {
  afterEach(() => {
    server.resetHandlers()
  })

  it('POSTs the budget body to /api/budget/:agentId and invalidates the budgets query', async () => {
    let getCalls = 0
    server.use(
      http.get('/api/budget', () => {
        getCalls += 1
        return HttpResponse.json(sampleResponse)
      }),
    )

    let postUrl: string | null = null
    let postBody: unknown = null
    server.use(
      http.post('/api/budget/:agentId', async ({ request, params }) => {
        postUrl = new URL(request.url).pathname
        postBody = await request.json()
        return HttpResponse.json({ ok: true, agentId: params.agentId })
      }),
    )

    const wrapper = createWrapper()
    // Prime the cache so we can detect the refetch that invalidation triggers.
    const list = renderHook(() => useBudgetList(), { wrapper })
    await waitFor(() => expect(list.result.current.isSuccess).toBe(true))
    expect(getCalls).toBe(1)

    const setter = renderHook(() => ({ set: useBudgetSet() }), { wrapper })
    const body: SetBudgetRequest = {
      token_budget: 200_000,
      calls_budget: 500,
      window_secs: 1800,
    }

    await act(async () => {
      await setter.result.current.set.mutateAsync({ agentId: 'agent-1', ...body })
    })

    // Endpoint + payload driven by production code, not duplicated in the test.
    expect(postUrl).toBe('/api/budget/agent-1')
    expect(postBody).toEqual(body)

    // onSuccess invalidates ['budgets'] → GET fires again.
    await waitFor(() => expect(getCalls).toBeGreaterThanOrEqual(2))
  })
})

describe('useBudgetDelete', () => {
  afterEach(() => {
    server.resetHandlers()
  })

  it('DELETEs /api/budget/:agentId and invalidates the budgets query', async () => {
    let getCalls = 0
    server.use(
      http.get('/api/budget', () => {
        getCalls += 1
        return HttpResponse.json(sampleResponse)
      }),
    )

    let deleteUrl: string | null = null
    let deleteMethod: string | null = null
    server.use(
      http.delete('/api/budget/:agentId', ({ request }) => {
        deleteUrl = new URL(request.url).pathname
        deleteMethod = request.method
        return new HttpResponse(null, { status: 204 })
      }),
    )

    const wrapper = createWrapper()
    const list = renderHook(() => useBudgetList(), { wrapper })
    await waitFor(() => expect(list.result.current.isSuccess).toBe(true))
    expect(getCalls).toBe(1)

    const deleter = renderHook(() => ({ del: useBudgetDelete() }), { wrapper })

    await act(async () => {
      await deleter.result.current.del.mutateAsync('agent-2')
    })

    expect(deleteUrl).toBe('/api/budget/agent-2')
    expect(deleteMethod).toBe('DELETE')
    await waitFor(() => expect(getCalls).toBeGreaterThanOrEqual(2))
  })
})

describe('useBudgetReset', () => {
  afterEach(() => {
    server.resetHandlers()
  })

  it('POSTs /api/budget/:agentId/reset and invalidates the budgets query', async () => {
    let getCalls = 0
    server.use(
      http.get('/api/budget', () => {
        getCalls += 1
        return HttpResponse.json(sampleResponse)
      }),
    )

    let resetUrl: string | null = null
    let resetMethod: string | null = null
    server.use(
      http.post('/api/budget/:agentId/reset', ({ request }) => {
        resetUrl = new URL(request.url).pathname
        resetMethod = request.method
        return HttpResponse.json({ ok: true })
      }),
    )

    const wrapper = createWrapper()
    const list = renderHook(() => useBudgetList(), { wrapper })
    await waitFor(() => expect(list.result.current.isSuccess).toBe(true))
    expect(getCalls).toBe(1)

    const resetter = renderHook(() => ({ reset: useBudgetReset() }), { wrapper })

    await act(async () => {
      await resetter.result.current.reset.mutateAsync('agent-1')
    })

    expect(resetUrl).toBe('/api/budget/agent-1/reset')
    expect(resetMethod).toBe('POST')
    await waitFor(() => expect(getCalls).toBeGreaterThanOrEqual(2))
  })

  it('surfaces the API error and does NOT refetch when the reset POST fails', async () => {
    let getCalls = 0
    server.use(
      http.get('/api/budget', () => {
        getCalls += 1
        return HttpResponse.json(sampleResponse)
      }),
    )
    server.use(
      http.post('/api/budget/:agentId/reset', () => new HttpResponse('boom', { status: 500 })),
    )

    const wrapper = createWrapper()
    const list = renderHook(() => useBudgetList(), { wrapper })
    await waitFor(() => expect(list.result.current.isSuccess).toBe(true))
    expect(getCalls).toBe(1)

    const resetter = renderHook(() => ({ reset: useBudgetReset() }), { wrapper })

    // Catch the rejection locally so the act() block can exit; the query
    // state updates that follow happen on subsequent microtasks.
    await act(async () => {
      await resetter.result.current.reset.mutateAsync('agent-1').catch(() => undefined)
    })

    await waitFor(() => expect(resetter.result.current.reset.isError).toBe(true))
    // onSuccess skipped on error → no invalidation → no refetch.
    expect(getCalls).toBe(1)
  })
})
