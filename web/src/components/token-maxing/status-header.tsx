import { Activity, Cpu, Timer } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { useTokenMaxingStatus } from '@/hooks/use-token-maxing'

/** Status header — running flag, current provider + task, this-session totals,
 *  active window (or "수동(Manual)"). Mirrors the `Card` + `Badge` pattern
 *  used by CostSummaryCards / ProviderQuotaCards.
 */
export function TokenMaxingStatusHeader() {
  const { t } = useTranslation()
  const { data, isLoading } = useTokenMaxingStatus()

  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
        <CardTitle className="flex items-center gap-2 text-base">
          <Activity className="h-4 w-4" />
          {t('tokenMaxing.status.title')}
        </CardTitle>
        <SessionStateBadge running={data?.running ?? false} />
      </CardHeader>
      <CardContent>
        {isLoading ? (
          <p className="text-sm text-muted-foreground py-2">{t('common.loading')}</p>
        ) : (
          <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
            <Field
              label={t('tokenMaxing.status.currentProvider')}
              value={data?.current_provider ?? t('tokenMaxing.status.noneActive')}
              icon={<Cpu className="h-4 w-4 text-muted-foreground" />}
            />
            <Field
              label={t('tokenMaxing.status.currentTask')}
              value={data?.current_task ?? t('tokenMaxing.status.noActiveTask')}
            />
            <Field
              label={t('tokenMaxing.status.tokensThisSession')}
              value={(data?.tokens_this_session ?? 0).toLocaleString()}
            />
            <Field
              label={t('tokenMaxing.status.tasksThisSession')}
              value={(data?.tasks_this_session ?? 0).toLocaleString()}
            />
            <WindowField window={data?.window ?? null} manual={data?.manual ?? false} />
          </div>
        )}
      </CardContent>
    </Card>
  )
}

function SessionStateBadge({ running }: { running: boolean }) {
  const { t } = useTranslation()
  return (
    <Badge variant={running ? 'success' : 'secondary'}>
      {running ? t('tokenMaxing.status.running') : t('tokenMaxing.status.idle')}
    </Badge>
  )
}

function Field({ label, value, icon }: { label: string; value: string; icon?: React.ReactNode }) {
  return (
    <div className="space-y-1">
      <div className="flex items-center gap-2 text-xs text-muted-foreground">
        {icon}
        <span>{label}</span>
      </div>
      <p className="text-sm font-medium">{value}</p>
    </div>
  )
}

function WindowField({
  window,
  manual,
}: {
  window: { start: string; end: string } | null
  manual: boolean
}) {
  const { t } = useTranslation()
  let value: string
  if (manual) {
    value = t('tokenMaxing.status.manual')
  } else if (window) {
    value = `${formatDateTime(window.start)} → ${formatDateTime(window.end)}`
  } else {
    value = t('tokenMaxing.status.noWindow')
  }
  return (
    <div className="space-y-1 sm:col-span-2 lg:col-span-4">
      <div className="flex items-center gap-2 text-xs text-muted-foreground">
        <Timer className="h-4 w-4" />
        <span>{t('tokenMaxing.status.window')}</span>
      </div>
      <p className="text-sm font-medium">{value}</p>
    </div>
  )
}

function formatDateTime(iso: string): string {
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return iso
  return d.toLocaleString()
}
