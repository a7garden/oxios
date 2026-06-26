import { createFileRoute } from '@tanstack/react-router'
import { useTranslation } from 'react-i18next'
import { TokenMaxingControls } from '@/components/token-maxing/controls'
import { TokenMaxingProviderCards } from '@/components/token-maxing/provider-cards'
import { TokenMaxingSessions } from '@/components/token-maxing/sessions'
import { TokenMaxingStatusHeader } from '@/components/token-maxing/status-header'

export const Route = createFileRoute('/token-maxing')({
  component: TokenMaxingPage,
})

/** Token Maxing (RFC-031) panel — under the Cost area in the sidebar.
 *  Composes the live status header, start/stop controls, per-provider
 *  availability verdicts, and the past-sessions report list.
 */
function TokenMaxingPage() {
  const { t } = useTranslation()
  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">{t('tokenMaxing.title')}</h1>
          <p className="text-sm text-muted-foreground">{t('tokenMaxing.subtitle')}</p>
        </div>
      </div>

      <TokenMaxingStatusHeader />
      <TokenMaxingControls />
      <TokenMaxingProviderCards />
      <TokenMaxingSessions />
    </div>
  )
}
