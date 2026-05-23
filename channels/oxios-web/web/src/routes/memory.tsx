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
    queryFn: async () => {
      const res = await api.get<{ items: { name: string; category: string }[] }>('/api/memory')
      // List endpoint returns name + category only — show category as content
      return (res.items ?? []).map((m) => ({ name: m.name, content: m.category, updated_at: '' }))
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
                <CardTitle className="text-sm flex items-center justify-between">
                  <span className="flex items-center gap-2">
                    <Brain className="h-3 w-3" /> {mem.name}
                  </span>
                  {mem.updated_at && (
                    <span className="text-xs text-muted-foreground font-normal">
                      {new Date(mem.updated_at).toLocaleString()}
                    </span>
                  )}
                </CardTitle>
              </CardHeader>
              <CardContent>
                <pre className="rounded bg-muted p-3 text-xs overflow-x-auto whitespace-pre-wrap">
                  {mem.content}
                </pre>
              </CardContent>
            </Card>
          ))}
        </div>
      )}
    </div>
  )
}
