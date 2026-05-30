import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { ArrowLeft, ChevronDown, ChevronRight, Dna, Target } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { ConstraintList } from '@/components/seed/constraint-list'
import { CriteriaList } from '@/components/seed/criteria-list'
import { EvaluationCard } from '@/components/seed/evaluation-card'
import { EvolutionChain } from '@/components/seed/evolution-chain'
import { LinkedAgents } from '@/components/seed/linked-agents'
import { OntologyGrid } from '@/components/seed/ontology-grid'
import { PhaseProgress } from '@/components/seed/phase-progress'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { api } from '@/lib/api-client'
import type { EvolutionEntry, OuroborosPhase, SeedDetail } from '@/types/seed'

export const Route = createFileRoute('/seeds/$seedId')({
  component: SeedDetailPage,
})

function SeedDetailPage() {
  const { t } = useTranslation()
  const { seedId } = Route.useParams()
  const navigate = useNavigate()
  const [showRaw, setShowRaw] = useState(false)

  const {
    data: seed,
    isLoading,
    isError,
    refetch,
  } = useQuery({
    queryKey: ['seed', seedId],
    queryFn: () => api.get<SeedDetail>(`/api/seeds/${seedId}`),
  })

  const { data: evolution } = useQuery({
    queryKey: ['seed', 'evolution', seedId],
    queryFn: () => api.get<EvolutionEntry[]>(`/api/seeds/${seedId}/evolution`),
    enabled: !!seedId,
  })

  if (isLoading) return <LoadingCards count={3} />
  if (isError) return <ErrorState onRetry={() => refetch()} />
  if (!seed) return <p className="text-muted-foreground">{t('seeds.notFound')}</p>

  const constraints = seed.constraints || []
  const criteria = seed.acceptance_criteria || []
  const ontology = seed.ontology || []
  const phaseReached = seed.phase_reached || 'seed'

  return (
    <div className="space-y-6">
      {/* Header */}
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
          <h1 className="flex items-center gap-2 text-2xl font-bold">
            <Dna className="h-6 w-6" /> {seed.goal || t('seeds.title')}
          </h1>
          <p className="font-mono text-xs text-muted-foreground">{seed.id}</p>
        </div>
        {seed.generation != null && (
          <Badge variant="default">
            {t('seeds.generation', { gen: seed.generation })}
          </Badge>
        )}
      </div>

      {/* Goal */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Target className="h-4 w-4" /> {t('seeds.goal')}
          </CardTitle>
        </CardHeader>
        <CardContent>
          <p className="text-sm">{seed.goal}</p>
        </CardContent>
      </Card>

      {/* Phase Progress */}
      <Card>
        <CardContent className="pt-6">
          <PhaseProgress phaseReached={phaseReached as OuroborosPhase} />
          <p className="mt-2 text-xs text-muted-foreground">
            {t('seeds.phaseReached')}: {t(`seeds.${phaseReached}`)}
          </p>
        </CardContent>
      </Card>

      {/* Constraints & Criteria */}
      <div className="grid gap-6 md:grid-cols-2">
        <ConstraintList constraints={constraints} />
        <CriteriaList criteria={criteria} />
      </div>

      {/* Ontology */}
      <OntologyGrid entities={ontology} />

      {/* Evaluation */}
      <EvaluationCard evaluation={seed.evaluation} />

      {/* Evolution */}
      {evolution && evolution.length > 0 && (
        <EvolutionChain
          entries={evolution}
          currentId={seed.id}
          onNavigate={(id) =>
            navigate({ to: '/seeds/$seedId', params: { seedId: id } })
          }
        />
      )}

      {/* Linked Agents */}
      <LinkedAgents seedId={seed.id} />

      {/* Raw Data */}
      <Card>
        <CardHeader>
          <CardTitle
            className="flex cursor-pointer items-center gap-2"
            onClick={() => setShowRaw(!showRaw)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' || e.key === ' ') setShowRaw(!showRaw)
            }}
            role="button"
            tabIndex={0}
          >
            {showRaw ? (
              <ChevronDown className="h-4 w-4" />
            ) : (
              <ChevronRight className="h-4 w-4" />
            )}
            {t('seeds.rawData')}
          </CardTitle>
        </CardHeader>
        {showRaw && (
          <CardContent>
            <pre className="overflow-x-auto rounded-lg bg-muted p-4 text-xs">
              {JSON.stringify(seed, null, 2)}
            </pre>
          </CardContent>
        )}
      </Card>
    </div>
  )
}
