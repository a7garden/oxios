import { useQuery } from '@tanstack/react-query'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { ArrowLeft, Dna } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { api } from '@/lib/api-client'

interface SeedDetail {
  id: string
  goal: string
  constraints: string[]
  created_at: string
  parent_seed_id?: string
  generation?: number
  [key: string]: unknown
}

export const Route = createFileRoute('/seeds/$seedId')({
  component: SeedDetailPage,
})

function SeedDetailPage() {
  const { t } = useTranslation()
  const { seedId } = Route.useParams()
  const navigate = useNavigate()

  const {
    data: seed,
    isLoading,
    isError,
    refetch,
  } = useQuery({
    queryKey: ['seed', seedId],
    queryFn: () => api.get<SeedDetail>(`/api/seeds/${seedId}`),
  })

  if (isLoading) return <LoadingCards count={3} />
  if (isError) return <ErrorState onRetry={() => refetch()} />
  if (!seed) return <p className="text-muted-foreground">{t('seeds.notFound')}</p>

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-4">
        <Button
          variant="ghost"
          size="icon"
          onClick={() => navigate({ to: '/seeds' })}
          aria-label={t('common.back')}
        >
          <ArrowLeft className="h-4 w-4" />
        </Button>
        <div className="flex-1">
          <h1 className="text-2xl font-bold flex items-center gap-2">
            <Dna className="h-6 w-6" /> {seed.goal}
          </h1>
          <p className="text-muted-foreground font-mono text-xs">{seed.id}</p>
        </div>
        {seed.generation != null && (
          <Badge variant="default">
            {t('seeds.generation', { gen: seed.generation })}
          </Badge>
        )}
      </div>

      <Card>
        <CardHeader>
          <CardTitle>{t('seeds.details')}</CardTitle>
        </CardHeader>
        <CardContent>
          <pre className="rounded-lg bg-muted p-4 text-xs overflow-x-auto">
            {JSON.stringify(seed, null, 2)}
          </pre>
        </CardContent>
      </Card>

      {seed.constraints && seed.constraints.length > 0 && (
        <Card>
          <CardHeader>
            <CardTitle>{t('seeds.constraints')}</CardTitle>
          </CardHeader>
          <CardContent>
            <ul className="space-y-1">
              {seed.constraints.map((c, i) => (
                <li key={i} className="text-sm flex items-start gap-2">
                  <span className="text-muted-foreground">•</span>
                  <span>{c}</span>
                </li>
              ))}
            </ul>
          </CardContent>
        </Card>
      )}
    </div>
  )
}
