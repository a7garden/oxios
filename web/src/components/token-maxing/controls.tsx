import { AlertCircle, AlertTriangle, Hand, Play, Square } from 'lucide-react'
import { useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import {
  useTokenMaxingProviders,
  useTokenMaxingStart,
  useTokenMaxingStatus,
  useTokenMaxingStop,
} from '@/hooks/use-token-maxing'

/** Start/Stop controls + window picker. Mirrors the Card+Button+Input pattern
 *  used by SetBudgetDialog and the cron-jobs create form.
 */
export function TokenMaxingControls() {
  const { t } = useTranslation()
  const { data: status } = useTokenMaxingStatus()
  const { data: providers } = useTokenMaxingProviders()
  const startMutation = useTokenMaxingStart()
  const stopMutation = useTokenMaxingStop()

  const running = status?.running ?? false
  const enabled = providers?.enabled ?? false

  // Default window: now → now + 5h (matches the ZAI 5h reset window in the
  // example config). The user can edit either field before starting.
  const defaultStart = useMemo(() => toLocalInput(new Date()), [])
  const defaultEnd = useMemo(() => {
    const d = new Date()
    d.setHours(d.getHours() + 5)
    return toLocalInput(d)
  }, [])
  const [start, setStart] = useState(defaultStart)
  const [end, setEnd] = useState(defaultEnd)

  // Pending start action — captures which variant the user clicked so we
  // can fire the right mutation after the metered-provider confirm dialog.
  const [pending, setPending] = useState<StartRequest | null>(null)

  const meteredProviders = (providers?.providers ?? []).filter((p) => p.billing_model === 'metered')
  const hasMetered = meteredProviders.length > 0

  const guardedStart = (req: StartRequest) => {
    if (hasMetered) {
      setPending(req)
      return
    }
    startMutation.mutate(req)
  }

  const handleStartWindow = () => {
    const startIso = fromLocalInput(start)
    const endIso = fromLocalInput(end)
    if (!startIso || !endIso) return
    if (new Date(endIso) <= new Date(startIso)) return
    guardedStart({ window: { start: startIso, end: endIso } })
  }

  const handleStartManual = () => {
    guardedStart({ manual: true })
  }

  const handleConfirmMetered = () => {
    if (!pending) return
    startMutation.mutate(pending)
    setPending(null)
  }

  const handleCancelMetered = () => {
    setPending(null)
  }

  const handleStop = () => {
    stopMutation.mutate()
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2 text-base">
          <Play className="h-4 w-4" />
          {t('tokenMaxing.controls.title')}
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        {!enabled && (
          <div className="flex items-start gap-2 rounded-md border border-warning-muted bg-warning-muted/30 p-3 text-sm">
            <AlertCircle className="h-4 w-4 mt-0.5 text-warning shrink-0" />
            <p className="text-warning">{t('tokenMaxing.controls.noEnabledProviders')}</p>
          </div>
        )}

        <div className="grid gap-3 sm:grid-cols-2">
          <div className="space-y-1">
            <label className="text-xs text-muted-foreground">
              {t('tokenMaxing.controls.windowStart')}
            </label>
            <Input
              type="datetime-local"
              value={start}
              onChange={(e) => setStart(e.target.value)}
              disabled={running}
            />
          </div>
          <div className="space-y-1">
            <label className="text-xs text-muted-foreground">
              {t('tokenMaxing.controls.windowEnd')}
            </label>
            <Input
              type="datetime-local"
              value={end}
              onChange={(e) => setEnd(e.target.value)}
              disabled={running}
            />
          </div>
        </div>

        <div className="flex flex-wrap gap-2">
          <Button
            onClick={handleStartWindow}
            disabled={running || !enabled || startMutation.isPending || !start || !end}
          >
            <Play className="h-4 w-4" />
            {t('tokenMaxing.controls.startWindow')}
          </Button>
          <Button
            variant="secondary"
            onClick={handleStartManual}
            disabled={running || !enabled || startMutation.isPending}
          >
            <Hand className="h-4 w-4" />
            {t('tokenMaxing.controls.startManual')}
          </Button>
          <Button
            variant="destructive"
            onClick={handleStop}
            disabled={!running || stopMutation.isPending}
          >
            <Square className="h-4 w-4" />
            {t('tokenMaxing.controls.stop')}
          </Button>
        </div>

        {startMutation.isError && (
          <p className="text-sm text-error">
            {t('tokenMaxing.controls.startFailed', {
              error: (startMutation.error as Error | null)?.message ?? '',
            })}
          </p>
        )}
        {stopMutation.isError && (
          <p className="text-sm text-error">
            {t('tokenMaxing.controls.stopFailed', {
              error: (stopMutation.error as Error | null)?.message ?? '',
            })}
          </p>
        )}
      </CardContent>

      <Dialog open={pending != null} onOpenChange={(o) => !o && handleCancelMetered()}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              <AlertTriangle className="h-4 w-4 text-warning" />
              {t('tokenMaxing.billing.meteredWarningTitle')}
            </DialogTitle>
            <DialogDescription>{t('tokenMaxing.billing.meteredWarning')}</DialogDescription>
          </DialogHeader>
          {meteredProviders.length > 0 && (
            <ul className="text-sm text-muted-foreground list-disc pl-5 space-y-1">
              {meteredProviders.map((p) => (
                <li key={p.provider}>{p.provider}</li>
              ))}
            </ul>
          )}
          <DialogFooter>
            <Button variant="outline" onClick={handleCancelMetered}>
              {t('common.cancel')}
            </Button>
            <Button onClick={handleConfirmMetered} disabled={startMutation.isPending}>
              {t('common.confirm')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </Card>
  )
}

/** Payload the controls will hand to `useTokenMaxingStart().mutate`. */
type StartRequest = { window: { start: string; end: string } } | { manual: true }

/** Format a Date as the `YYYY-MM-DDTHH:mm` value that `<input type="datetime-local">` expects. */
function toLocalInput(d: Date): string {
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`
}

/** Convert `<input type="datetime-local">` value back to ISO-8601 UTC. */
function fromLocalInput(local: string): string | null {
  const d = new Date(local)
  if (Number.isNaN(d.getTime())) return null
  return d.toISOString()
}
