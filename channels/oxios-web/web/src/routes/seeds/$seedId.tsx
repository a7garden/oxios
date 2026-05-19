import { useQuery } from '@tanstack/react-query'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { ArrowLeft, Dna, GitCommit } from 'lucide-react'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { api } from '@/lib/api-client'
import type { Seed } from '@/types'

export const Route = createFileRoute('/seeds/$seedId')({
  component: SeedDetailPage,
})

function SeedDetailPage() {
  const { seedId } = Route.useParams()
  const navigate = useNavigate()

  const { data: seed, isLoading, isError, refetch } = useQuery({
    queryKey: ['seed', seedId],
    queryFn: () => api.get<Seed>(`/api/seeds/${seedId}`),
  })

  if (isLoading) return <LoadingCards count={3} />
  if (isError) return <ErrorState onRetry={() => refetch()} />
  if (!seed) return <p className="text-muted-foreground">Seed not found.</p>

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" onClick={() => navigate({ to: '/seeds' })} aria-label="Go back">
          <ArrowLeft className="h-4 w-4" />
        </Button>
        <div className="flex-1">
          <h1 className="text-2xl font-bold flex items-center gap-2">
            <Dna className="h-6 w-6" /> {seed.name}
          </h1>
          <p className="text-muted-foreground font-mono text-xs">{seed.id}</p>
        </div>
        <Badge variant="default">{seed.phase}</Badge>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Spec</CardTitle>
        </CardHeader>
        <CardContent>
          <pre className="rounded-lg bg-muted p-4 text-xs overflow-x-auto">
            {JSON.stringify(seed.spec, null, 2)}
          </pre>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <GitCommit className="h-4 w-4" /> Evolution Timeline
          </CardTitle>
        </CardHeader>
        <CardContent>
          {seed.evolution_log && seed.evolution_log.length > 0 ? (
            <div className="relative space-y-0">
              {seed.evolution_log.map((entry, i) => (
                <div
                  // biome-ignore lint/suspicious/noArrayIndexKey: evolution entries have no unique ID
                  key={`evolution-${i}`}
                  className="flex gap-4 pb-6 relative"
                >
                  <div className="flex flex-col items-center">
                    <div className="h-3 w-3 rounded-full bg-primary mt-1" />
                    {i < (seed.evolution_log?.length ?? 0) - 1 && (
                      <div className="w-0.5 flex-1 bg-border" />
                    )}
                  </div>
                  <div className="flex-1">
                    <div className="flex items-center gap-2 mb-1">
                      <Badge variant="secondary" className="text-xs">
                        {entry.phase}
                      </Badge>
                      <span className="text-xs text-muted-foreground">
                        {new Date(entry.timestamp).toLocaleString()}
                      </span>
                    </div>
                    <p className="text-sm">{entry.summary}</p>
                    {entry.changes && (
                      <pre className="mt-2 rounded bg-muted p-2 text-xs overflow-x-auto">
                        {JSON.stringify(entry.changes, null, 2)}
                      </pre>
                    )}
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <p className="text-sm text-muted-foreground">No evolution entries yet.</p>
          )}
        </CardContent>
      </Card>
    </div>
  )
}
