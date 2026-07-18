import { createFileRoute } from '@tanstack/react-router'
import { useTranslation } from 'react-i18next'
import { PageHeader } from '@/components/shared/page-header'
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
      <PageHeader
        title={t('tokenMaxing.title')}
        subtitle={t('tokenMaxing.subtitle')}
        titleMeta={<span className="text-xs text-muted-foreground">RFC-031</span>}
      />

      <TokenMaxingStatusHeader />
      <TokenMaxingControls />
      <TokenMaxingProviderCards />
      <TokenMaxingSessions />
    </div>
  )
}
