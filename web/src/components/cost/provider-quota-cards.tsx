import { useTranslation } from 'react-i18next'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { useProviderQuotas } from '@/hooks/use-costs'
import { useProviders } from '@/hooks/use-engine'
import { AlertCircle, CheckCircle2, Wallet } from 'lucide-react'
import type { ProviderInfo } from '@/types/engine'
import type { QuotaSnapshot } from '@/types/cost'

/** Provider panel — shows ALL configured providers, merged with external
 * quota/billing data where available.
 *
 * Previously this component only showed data from `/api/costs/providers`
 * (external billing API calls). That meant providers with API keys set but
 * no billing endpoint (or a failed fetch) were invisible. Now we always
 * show configured providers from `/api/engine/providers` and overlay quota
 * data as a bonus when the external API is reachable.
 */
export function ProviderQuotaCards() {
  const { t } = useTranslation()
  const { data: quotaData, isLoading: quotaLoading } = useProviderQuotas()
  const { data: providers } = useProviders()

  const quotas = quotaData?.providers ?? []
  const configured = providers ?? []

  // Merge: keyed by provider id. Configured providers always show; quota
  // data is attached where the external fetch succeeded.
  const quotaMap = new Map<string, QuotaSnapshot>()
  for (const q of quotas) quotaMap.set(q.provider, q)

  const merged: { info: ProviderInfo; quota: QuotaSnapshot | null }[] = configured.map(
    (info) => ({ info, quota: quotaMap.get(info.id) ?? null }),
  )

  // Also include quota-only providers (fetcher found a key but provider
  // isn't in the engine catalog — rare, but shouldn't be hidden).
  for (const q of quotas) {
    if (!configured.some((p) => p.id === q.provider)) {
      merged.push({
        info: {
          id: q.provider,
          name: q.provider,
          category: 'major',
          hasKey: true,
          modelCount: 0,
        },
        quota: q,
      })
    }
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-base">
          <Wallet className="h-4 w-4" />
          {t('cost.providerQuota')}
        </CardTitle>
      </CardHeader>
      <CardContent>
        {quotaLoading && merged.length === 0 ? (
          <p className="text-sm text-muted-foreground py-4">{t('common.loading')}</p>
        ) : merged.length === 0 ? (
          <p className="text-sm text-muted-foreground py-4">{t('cost.noProviderQuotaDesc')}</p>
        ) : (
          <div className="space-y-3">
            {merged.map(({ info, quota }) => (
              <ProviderRow key={info.id} info={info} quota={quota} />
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  )
}

function ProviderRow({ info, quota }: { info: ProviderInfo; quota: QuotaSnapshot | null }) {
  const { t } = useTranslation()

  return (
    <div className="flex items-center justify-between rounded-lg border p-3">
      <div className="space-y-1">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium">{info.name}</span>
          {info.hasKey ? (
            <Badge variant="default" className="gap-1 text-xs">
              <CheckCircle2 className="h-3 w-3" />
              {info.keySource ?? 'configured'}
            </Badge>
          ) : (
            <Badge variant="outline" className="text-xs">
              {t('cost.noKey')}
            </Badge>
          )}
          {info.modelCount > 0 && (
            <span className="text-xs text-muted-foreground">
              {info.modelCount} {t('cost.models')}
            </span>
          )}
          {quota?.plan && (
            <Badge variant="secondary" className="text-xs">
              {quota.plan}
            </Badge>
          )}
        </div>

        {quota?.error ? (
          <div className="flex items-center gap-1 text-xs text-muted-foreground">
            <AlertCircle className="h-3 w-3" />
            {quota.error}
          </div>
        ) : quota ? (
          <div className="flex items-center gap-4 text-xs text-muted-foreground">
            {quota.period_spend_usd != null && (
              <span>
                {t('cost.periodSpend')}: ${quota.period_spend_usd.toFixed(2)}
              </span>
            )}
            {quota.credit_balance_usd != null && (
              <span>
                {t('cost.balance')}: ${quota.credit_balance_usd.toFixed(2)}
              </span>
            )}
          </div>
        ) : (
          <p className="text-xs text-muted-foreground">{t('cost.quotaUnavailable')}</p>
        )}
      </div>
    </div>
  )
}
