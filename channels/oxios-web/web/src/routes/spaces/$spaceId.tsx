import { useQuery } from '@tanstack/react-query'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { ArrowLeft, Boxes } from 'lucide-react'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { api } from '@/lib/api-client'
import type { Space } from '@/types'

export const Route = createFileRoute('/spaces/$spaceId')({
  component: SpaceDetailPage,
})

function SpaceDetailPage() {
  const { spaceId } = Route.useParams()
  const navigate = useNavigate()

  const {
    data: space,
    isLoading,
    isError,
    refetch,
  } = useQuery({
    queryKey: ['space', spaceId],
    queryFn: () => api.get<Space>(`/api/spaces/${spaceId}`),
  })

  if (isLoading) return <LoadingCards count={3} />
  if (isError) return <ErrorState onRetry={() => refetch()} />
  if (!space) return <p className="text-muted-foreground">Space not found.</p>

  const details = [
    { label: 'ID', value: space.id },
    { label: 'Name', value: space.name },
    { label: 'Source', value: space.source ?? '—' },
    { label: 'Tags', value: space.tags && space.tags.length > 0 ? space.tags.join(', ') : '—' },
    { label: 'Active', value: space.active !== false ? 'Yes' : 'No' },
    { label: 'Created', value: new Date(space.created_at).toLocaleString() },
    ...(space.last_active_at ? [{ label: 'Last Active', value: new Date(space.last_active_at).toLocaleString() }] : []),
    ...(space.interaction_count != null ? [{ label: 'Interactions', value: String(space.interaction_count) }] : []),
  ]

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button
          variant="ghost"
          size="icon"
          onClick={() => navigate({ to: '/spaces' })}
          aria-label="Go back"
        >
          <ArrowLeft className="h-4 w-4" />
        </Button>
        <div className="flex-1">
          <h1 className="text-2xl font-bold flex items-center gap-2">
            <Boxes className="h-6 w-6" /> {space.name}
          </h1>
          <p className="text-muted-foreground">Space Detail</p>
        </div>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>Space Information</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="grid gap-3 md:grid-cols-2">
            {details.map((d) => (
              <div
                key={d.label}
                className="flex items-center justify-between rounded-lg border p-3"
              >
                <span className="text-sm text-muted-foreground">{d.label}</span>
                <span className="text-sm font-medium">{d.value}</span>
              </div>
            ))}
          </div>
        </CardContent>
      </Card>

      {space.paths && space.paths.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle>Paths</CardTitle>
          </CardHeader>
          <CardContent>
            <ul className="space-y-1">
              {space.paths.map((p) => (
                <li key={p} className="text-sm font-mono text-muted-foreground">{p}</li>
              ))}
            </ul>
          </CardContent>
        </Card>
      )}
    </div>
  )
}
