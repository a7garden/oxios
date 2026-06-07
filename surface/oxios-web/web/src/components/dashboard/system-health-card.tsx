import { useNavigate } from '@tanstack/react-router'
import { Settings, Shield, Sparkles } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Progress } from '@/components/ui/progress'
import { Separator } from '@/components/ui/separator'
import { useEngineConfig, useProviders, useRoutingStats } from '@/hooks/use-engine'
import type { SystemStatus } from '@/types'

export interface SystemHealthCardProps {
  status?: SystemStatus
}

/**
 * Combined system health card for the dashboard.
 *
 * Replaces the three separate `SystemHealthCard`, `CurrentModelCard`,
 * and `ModelUsageCard` components from the old dashboard. The new
 * layout shows:
 *
 *   ┌─ Shield · 시스템 상태        v0.14.2 ─┐
 *   │                                      │
 *   │   💡 모델: gpt-4o  [openai]  [⚙️]   │  ← CurrentModelCard
 *   │                                      │
 *   │   ✅ Store   ✅ Bus                 │  ← HealthRow
 *   │   ✅ Memory (142)   ⏱ 1h 30m 5s     │
 *   │   12 완료 · 2 실패                   │  ← agents summary
 *   │  ─────────────────────────────────  │
 *   │   모델 사용량                       │  ← ModelUsageCard
 *   │   gpt-4o     65% · $1.23            │
 *   │   $1.68 총 비용 · 342회 호출        │
 *   └──────────────────────────────────────┘
 */
export function SystemHealthCard({ status }: SystemHealthCardProps) {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const { data: engineConfig } = useEngineConfig()
  const { data: providers } = useProviders()
  const { data: routingStats } = useRoutingStats()

  if (!status) return null

  const currentModel = engineConfig?.default_model ?? ''
  const providerId = currentModel.includes('/') ? currentModel.split('/')[0] : null
  const modelId = currentModel.includes('/')
    ? currentModel.split('/').slice(1).join('/')
    : currentModel
  const providerName = providers?.find((p) => p.id === providerId)?.name ?? providerId ?? '—'

  const completed = status.components?.agents?.total_completed ?? 0
  const failed = status.components?.agents?.total_failed ?? 0
  const hasUsage = !!routingStats && routingStats.totalRequests > 0

  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between pb-2">
        <CardTitle className="flex items-center gap-2 text-base">
          <Shield className="h-4 w-4" />
          {t('dashboard.systemHealth')}
        </CardTitle>
        <span className="text-xs font-mono text-muted-foreground">{status.version}</span>
      </CardHeader>
      <CardContent className="pt-0 space-y-3">
        {/* ── Model ── */}
        <button
          type="button"
          onClick={() => navigate({ to: '/settings', search: { section: 'engine' } })}
          className="flex w-full items-center gap-3 rounded-md p-2 -mx-2 text-left hover:bg-accent/40 transition-colors group"
        >
          <Sparkles className="h-4 w-4 shrink-0 text-violet-500" />
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-2">
              <span className="text-sm font-semibold truncate">
                {modelId || t('dashboard.modelNotSet')}
              </span>
              {providerId && (
                <span className="inline-flex items-center rounded-full bg-primary/10 px-2 py-0.5 text-xs font-medium text-primary whitespace-nowrap">
                  {providerName}
                </span>
              )}
            </div>
            {currentModel && (
              <p className="text-2xs text-muted-foreground mt-0.5 font-mono truncate">
                {currentModel}
              </p>
            )}
          </div>
          <Settings className="h-4 w-4 text-muted-foreground group-hover:text-foreground transition-colors" />
        </button>

        {/* ── Health rows ── */}
        <div className="grid gap-2 sm:grid-cols-2 text-xs">
          {status.components?.state_store && (
            <HealthRow
              label={t('dashboard.stateStore')}
              healthy={status.components.state_store.healthy}
              detail={status.components.state_store.detail}
            />
          )}
          {status.components?.event_bus && (
            <HealthRow
              label={t('dashboard.eventBus')}
              healthy={status.components.event_bus.healthy}
              detail={status.components.event_bus.detail}
            />
          )}
          {status.components?.memory && (
            <HealthRow
              label={t('dashboard.memory')}
              healthy={status.components.memory.enabled}
              detail={t('dashboard.entriesIndexed', { count: status.components.memory.index_size })}
            />
          )}
          <div className="flex items-center gap-2 text-muted-foreground">
            <span className="text-foreground">⏱</span>
            <span className="text-foreground">{t('dashboard.uptime')}</span>
            <span className="truncate">{status.uptime}</span>
          </div>
        </div>

        {/* ── Agent completion/failure summary ── */}
        {(completed > 0 || failed > 0) && (
          <p className="text-xs text-muted-foreground">
            <span className="text-foreground">{completed}</span> {t('dashboard.agentsCompleted')}
            {failed > 0 && (
              <>
                {' · '}
                <span className="text-error font-medium">
                  {failed} {t('dashboard.agentsFailed')}
                </span>
              </>
            )}
          </p>
        )}

        {/* ── Model usage (RFC-011 routing stats) ── */}
        {hasUsage && (
          <>
            <Separator />
            <div className="space-y-2">
              <h3 className="text-xs font-semibold text-muted-foreground uppercase tracking-wide">
                {t('dashboard.modelUsage')}
              </h3>
              {Object.entries(routingStats!.modelCalls)
                .sort(([, a], [, b]) => b - a)
                .slice(0, 5)
                .map(([model, count]) => {
                  const pct = (count / routingStats!.totalRequests) * 100
                  const cost = routingStats!.modelCost[model] ?? 0
                  return (
                    <div key={model} className="space-y-1">
                      <div className="flex justify-between text-xs">
                        <span className="truncate max-w-[55%]" title={model}>
                          {model.split('/').pop()}
                        </span>
                        <span className="text-muted-foreground">
                          {pct.toFixed(0)}% ({count}) · ${cost.toFixed(3)}
                        </span>
                      </div>
                      <Progress value={pct} className="h-1.5" />
                    </div>
                  )
                })}
              {routingStats!.totalCost > 0 && (
                <p className="pt-1 text-xs text-muted-foreground">
                  {t('dashboard.totalCostAndCalls', {
                    cost: routingStats!.totalCost.toFixed(2),
                    count: routingStats!.totalRequests,
                  })}
                </p>
              )}
            </div>
          </>
        )}
      </CardContent>
    </Card>
  )
}

function HealthRow({
  label,
  healthy,
  detail,
}: {
  label: string
  healthy: boolean
  detail?: string | null
}) {
  return (
    <div className="flex items-center gap-2 text-muted-foreground">
      <div className={`h-2 w-2 rounded-full ${healthy ? 'bg-success' : 'bg-error'}`} />
      <span className="text-foreground">{label}</span>
      {detail && <span className="truncate text-xs">· {detail}</span>}
    </div>
  )
}
