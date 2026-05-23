import { useQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { Brain, RefreshCw, Search } from 'lucide-react'
import { useState } from 'react'
import { EmptyState } from '@/components/shared/empty-state'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { api } from '@/lib/api-client'

interface MemoryItem {
  name: string
  category?: string
  snippet?: string
  content?: string
}

export const Route = createFileRoute('/memory')({ component: MemoryPage })

function MemoryPage() {
  const [search, setSearch] = useState('')

  const {
    data: memories,
    isLoading,
    isError,
    refetch,
    isFetching,
  } = useQuery({
    queryKey: ['memory', search],
    queryFn: async (): Promise<MemoryItem[]> => {
      if (search.trim()) {
        // Use search endpoint when query is provided
        const res = await api.post<{
          entries?: {
            id: string
            type: string
            content: string
            tags: string[]
            importance: number
            created_at: string
          }[]
          count: number
        }>('/api/memory/search', { query: search })
        const entries = Array.isArray(res?.entries) ? res.entries : []
        return entries.map((e) => ({
          name: e.id ?? e.type,
          snippet: e.content?.slice(0, 200) ?? '',
          category: e.type,
        }))
      }
      // Default: list all memory entries
      const res = await api.get<{ items?: { name: string; category: string }[] }>('/api/memory')
      const items = Array.isArray(res?.items) ? res.items : []
      return items.map((m) => ({ name: m.name, category: m.category }))
    },
    refetchInterval: 15000,
  })

  if (isLoading) return <LoadingCards count={4} />
  if (isError) return <ErrorState onRetry={() => refetch()} />

  const items = memories ?? []

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Memory</h1>
          <p className="text-muted-foreground">Agent memory store</p>
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

      <div className="relative max-w-md">
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
        <Input
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="Search memories..."
          className="pl-9"
        />
      </div>

      {items.length === 0 ? (
        <EmptyState
          icon={<Brain className="h-10 w-10" />}
          title="No memories"
          description={
            search ? 'No results for your search.' : 'Memories will be stored as agents work.'
          }
        />
      ) : (
        <div className="space-y-3">
          {items.map((mem) => (
            <Card key={mem.name}>
              <CardHeader className="pb-2">
                <CardTitle className="text-sm flex items-center gap-2">
                  <Brain className="h-3 w-3" /> {mem.name}
                </CardTitle>
              </CardHeader>
              <CardContent>
                <p className="text-xs text-muted-foreground">
                  {mem.snippet ?? mem.category ?? '—'}
                </p>
              </CardContent>
            </Card>
          ))}
        </div>
      )}
    </div>
  )
}
