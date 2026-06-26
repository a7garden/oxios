import { AlertCircle, CheckCircle2, Clock, Wallet } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { useTokenMaxingProviders, useTokenMaxingStatus } from '@/hooks/use-token-maxing'
import {
  type AvailabilityVerdict,
  type NormalizedAvailability,
  normalizeAvailability,
} from '@/types/token-maxing'

/** Live provider availability panel — RFC-031 §4 verdict per provider.
 *  Mirrors ProviderQuotaCards structure (Card → list of rows with
 *  verdict badge + secondary line for counter details).
 */
export function TokenMaxingProviderCards() {
  const { t } = useTranslation()
  // /providers has the authoritative per-provider list and recalibration/
  // cooldown history; /status has the same providers plus live session info.
  // Either is fine — use /providers for this panel.
  const { data, isLoading } = useTokenMaxingProviders()
  const status = useTokenMaxingStatus()

  const providers = data?.providers ?? status.data?.providers ?? []

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-base">
          <Wallet className="h-4 w-4" />
          {t('tokenMaxing.providers.title')}
        </CardTitle>
      </CardHeader>
      <CardContent>
        {isLoading && providers.length === 0 ? (
          <p className="text-sm text-muted-foreground py-4">{t('common.loading')}</p>
        ) : providers.length === 0 ? (
          <p className="text-sm text-muted-foreground py-4">{t('tokenMaxing.providers.empty')}</p>
        ) : (
          <div className="space-y-3">
            {providers.map((p) => {
              const norm = normalizeAvailability(p.availability)
              return <ProviderRow key={p.provider} provider={p.provider} availability={norm} />
            })}
          </div>
        )}
      </CardContent>
    </Card>
  )
}

function ProviderRow({
  provider,
  availability,
}: {
  provider: string
  availability: NormalizedAvailability
}) {
  return (
    <div className="flex items-center justify-between rounded-lg border p-3">
      <div className="space-y-1 min-w-0">
        <div className="flex items-center gap-2">
          <span className="text-sm font-medium">{provider}</span>
          <VerdictBadge verdict={availability.verdict} />
        </div>
        <ProviderDetail availability={availability} />
      </div>
    </div>
  )
}

function VerdictBadge({ verdict }: { verdict: AvailabilityVerdict }) {
  const { t } = useTranslation()
  switch (verdict) {
    case 'available':
      return (
        <Badge variant="success" className="gap-1 text-xs">
          <CheckCircle2 className="h-3 w-3" />
          {t('tokenMaxing.providers.verdict.available')}
        </Badge>
      )
    case 'draining':
      return (
        <Badge variant="warning" className="gap-1 text-xs">
          <AlertCircle className="h-3 w-3" />
          {t('tokenMaxing.providers.verdict.draining')}
        </Badge>
      )
    case 'cooled_down':
      return (
        <Badge variant="error" className="gap-1 text-xs">
          <Clock className="h-3 w-3" />
          {t('tokenMaxing.providers.verdict.cooledDown')}
        </Badge>
      )
    default:
      return (
        <Badge variant="outline" className="text-xs">
          {t('tokenMaxing.providers.verdict.ineligible')}
        </Badge>
      )
  }
}

function ProviderDetail({ availability }: { availability: NormalizedAvailability }) {
  const { t } = useTranslation()
  const snap = availability.snapshot

  if (availability.verdict === 'ineligible') {
    return <p className="text-xs text-muted-foreground">{t('tokenMaxing.providers.notEligible')}</p>
  }

  if (availability.verdict === 'cooled_down') {
    return (
      <div className="flex items-center gap-3 text-xs text-muted-foreground">
        {availability.until && (
          <span>
            {t('tokenMaxing.providers.cooldownUntil', {
              time: formatDateTime(availability.until),
            })}
          </span>
        )}
        {availability.reason && (
          <span>
            {t('tokenMaxing.providers.cooldownReason', {
              reason: availability.reason,
            })}
          </span>
        )}
      </div>
    )
  }

  // Available or Draining — show self-tracked counter snapshot.
  if (!snap) {
    return <p className="text-xs text-muted-foreground">{t('tokenMaxing.providers.noSnapshot')}</p>
  }

  const remaining = snap.remaining_percent
  return (
    <div className="flex items-center gap-4 text-xs text-muted-foreground">
      <span>
        {t('tokenMaxing.providers.tokensUsed', {
          used: snap.tokens_used.toLocaleString(),
          limit: snap.token_limit.toLocaleString(),
        })}
      </span>
      {remaining != null && (
        <span>
          {t('tokenMaxing.providers.remaining', {
            percent: remaining.toFixed(1),
          })}
        </span>
      )}
      {snap.resets_at && (
        <span>
          {t('tokenMaxing.providers.resetsAt', {
            time: formatDateTime(snap.resets_at),
          })}
        </span>
      )}
    </div>
  )
}

function formatDateTime(iso: string): string {
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return iso
  return d.toLocaleString()
}
