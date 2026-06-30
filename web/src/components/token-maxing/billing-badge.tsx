import { CreditCard, Crown, HelpCircle } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Badge } from '@/components/ui/badge'
import type { BillingModel } from '@/types/token-maxing'

/** Small badge showing how a provider bills usage. Surfaces the answer to
 *  "why is this provider showing as Available/Draining?" — `metered` means
 *  running token-maxing against it will cost real money, `subscription` is
 *  the original capped-plan target, `unknown` means the live quota API
 *  didn't give us enough signal to classify.
 *
 *  Defensive at the seam: the wire payload is typed `BillingModel` but
 *  backends that pre-date RFC-031 v2 may omit the field — coerce missing
 *  values to `unknown` rather than render `undefined`.
 */
export function BillingBadge({ billingModel }: { billingModel: BillingModel | null | undefined }) {
  const { t } = useTranslation()
  const model: BillingModel = billingModel ?? 'unknown'

  switch (model) {
    case 'subscription':
      return (
        <Badge
          variant="success"
          className="gap-1 text-xs"
          title={t('tokenMaxing.billing.subscription')}
        >
          <Crown className="h-3 w-3" />
          {t('tokenMaxing.billing.subscription')}
        </Badge>
      )
    case 'metered':
      return (
        <Badge variant="error" className="gap-1 text-xs" title={t('tokenMaxing.billing.metered')}>
          <CreditCard className="h-3 w-3" />
          {t('tokenMaxing.billing.metered')}
        </Badge>
      )
    default:
      return (
        <Badge
          variant="outline"
          className="gap-1 text-xs"
          title={t('tokenMaxing.billing.unknownHelp')}
        >
          <HelpCircle className="h-3 w-3" />
          {t('tokenMaxing.billing.unknown')}
        </Badge>
      )
  }
}
