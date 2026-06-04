import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { renderHook, waitFor } from '@testing-library/react'
import { HttpResponse, http } from 'msw'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { useMemoryMap } from '@/hooks/use-memory'
import type { MemoryMapResponse } from '@/types/memory'
import { server } from '../msw/server'

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: 'en' },
  }),
}))

const createWrapper = () => {
  const qc = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } },
  })
  return ({ children }: { children: React.ReactNode }) => (
    <QueryClientProvider client={qc}>{children}</QueryClientProvider>
  )
}

const sampleResponse: MemoryMapResponse = {
  count: 3,
  epoch: 12345,
  entries: [
    {
      id: 'a',
      tier: 'hot',
      mem_type: 'fact',
      content_preview: 'first',
      created_at: '2026-06-04T00:00:00Z',
      access_count: 1,
      coords_2d: [0.1, 0.2],
      top_neighbors: [{ id: 'b', similarity: 0.8 }],
    },
    {
      id: 'b',
      tier: 'warm',
      mem_type: 'episode',
      content_preview: 'second',
      created_at: '2026-06-04T00:00:00Z',
      access_count: 2,
      coords_2d: [0.3, -0.4],
      top_neighbors: [],
    },
    {
      id: 'c',
      tier: 'cold',
      mem_type: 'decision',
      content_preview: 'third',
      created_at: '2026-06-04T00:00:00Z',
      access_count: 0,
      coords_2d: [-0.5, 0.0],
      top_neighbors: [],
    },
  ],
}

describe('useMemoryMap', () => {
  beforeEach(() => {
    server.use(
      http.get('/api/memory/map', ({ request }) => {
        const url = new URL(request.url)
        const tier = url.searchParams.get('tier')
        if (tier === 'hot') {
          return HttpResponse.json({
            count: 1,
            epoch: 12345,
            entries: [sampleResponse.entries[0]],
          })
        }
        return HttpResponse.json(sampleResponse)
      }),
    )
  })

  afterEach(() => {
    server.resetHandlers()
  })

  it('fetches all entries when no filter is given', async () => {
    const { result } = renderHook(() => useMemoryMap(), {
      wrapper: createWrapper(),
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data?.count).toBe(3)
    expect(result.current.data?.entries).toHaveLength(3)
  })

  it('passes tier filter to the query string', async () => {
    const { result } = renderHook(() => useMemoryMap({ tier: 'hot' }), {
      wrapper: createWrapper(),
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(result.current.data?.count).toBe(1)
    expect(result.current.data?.entries?.[0]?.tier).toBe('hot')
  })

  it('passes mem_type filter to the query string', async () => {
    // Regression for P0-1: the filter must reach the server as a
    // singular label (e.g. "fact", "episode"). The previous backend
    // compared the singular label to the plural category name and
    // silently returned 0 entries for every non-knowledge type.
    const lastQuery: { value: URL | null } = { value: null }
    server.use(
      http.get('/api/memory/map', ({ request }) => {
        const url = new URL(request.url)
        lastQuery.value = url
        const memType = url.searchParams.get('mem_type')
        return HttpResponse.json({
          count: memType === 'episode' ? 1 : 0,
          epoch: 12345,
          entries: memType === 'episode' ? [sampleResponse.entries[1]] : [],
        })
      }),
    )
    const { result } = renderHook(() => useMemoryMap({ mem_type: 'episode' }), {
      wrapper: createWrapper(),
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    expect(lastQuery.value?.searchParams.get('mem_type')).toBe('episode')
    expect(result.current.data?.count).toBe(1)
    expect(result.current.data?.entries?.[0]?.mem_type).toBe('episode')
  })

  it('exposes 2D coordinates for each entry', async () => {
    const { result } = renderHook(() => useMemoryMap(), {
      wrapper: createWrapper(),
    })
    await waitFor(() => expect(result.current.isSuccess).toBe(true))
    for (const e of result.current.data?.entries ?? []) {
      expect(e.coords_2d).toHaveLength(2)
      expect(typeof e.coords_2d?.[0]).toBe('number')
      expect(typeof e.coords_2d?.[1]).toBe('number')
    }
  })
})
