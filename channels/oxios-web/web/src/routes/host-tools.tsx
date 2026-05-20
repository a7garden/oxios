import { useQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { CheckCircle, RefreshCw, Wrench, XCircle } from 'lucide-react'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { api } from '@/lib/api-client'

interface ToolCheck {
  name: string
  available: boolean
  version?: string
  path?: string
}

export const Route = createFileRoute('/host-tools')({ component: HostToolsPage })

function HostToolsPage() {
  const { data, isLoading, isError, refetch, isFetching } = useQuery({
    queryKey: ['host-tools'],
    queryFn: () => api.get<ToolCheck[]>('/api/host-tools'),
  })

  if (isLoading) return <LoadingCards count={4} />
  if (isError) return <ErrorState onRetry={() => refetch()} />

  const tools = data ?? []
  const available = tools.filter((t) => t.available).length
  const unavailable = tools.filter((t) => !t.available).length

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Host Tools</h1>
          <p className="text-muted-foreground">Available host system tools and capabilities</p>
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

      <div className="grid gap-4 md:grid-cols-2">
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm text-muted-foreground">Available</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold text-emerald-500">{available}</div>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm text-muted-foreground">Unavailable</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold text-red-500">{unavailable}</div>
          </CardContent>
        </Card>
      </div>

      <div className="grid gap-3 md:grid-cols-2 lg:grid-cols-3">
        {tools.map((tool) => (
          <Card key={tool.name}>
            <CardContent className="flex items-center gap-3 p-4">
              <Wrench className="h-4 w-4 text-muted-foreground shrink-0" />
              <div className="flex-1 min-w-0">
                <p className="font-medium text-sm flex items-center gap-2">
                  {tool.name}
                  {tool.version && (
                    <span className="text-xs text-muted-foreground">v{tool.version}</span>
                  )}
                </p>
                {tool.path && (
                  <p className="text-xs text-muted-foreground font-mono truncate">{tool.path}</p>
                )}
              </div>
              {tool.available ? (
                <CheckCircle className="h-4 w-4 text-emerald-500 shrink-0" />
              ) : (
                <XCircle className="h-4 w-4 text-red-500 shrink-0" />
              )}
            </CardContent>
          </Card>
        ))}
      </div>
    </div>
  )
}
