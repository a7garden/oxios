import { AlertTriangle, Pencil, Target } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { useSetSpendLimit, useSpendLimit } from '@/hooks/use-costs'

/** Monthly spend limit card — shows a progress bar of month-to-date spend
 * against the user-configured limit, with an inline dialog to set or clear
 * the limit.
 *
 * Phase 1: monitoring + visual alert only. Phase 2 will add pre-execution
 * enforcement.
 */
export function SpendLimitCard() {
  const { t } = useTranslation()
  const { data, isLoading } = useSpendLimit()
  const [dialogOpen, setDialogOpen] = useState(false)

  const limit = data?.monthly_limit_usd ?? null
  const mtdSpend = data?.month_to_date_spend_usd ?? 0
  const pct = limit != null && limit > 0 ? Math.min(100, (mtdSpend / limit) * 100) : 0
  const isOver = limit != null && mtdSpend >= limit
  const isNear = limit != null && !isOver && pct >= 80

  return (
    <>
      <Card className={isOver ? 'border-error/50' : isNear ? 'border-warning/50' : ''}>
        <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
          <CardTitle className="flex items-center gap-2 text-sm font-medium">
            <Target className="h-4 w-4" />
            {t('cost.spendLimit')}
            <span className="text-2xs font-normal text-muted-foreground">{t('cost.monitoringOnly')}</span>
          </CardTitle>
          {limit != null ? (
            <Badge variant={isOver ? 'destructive' : isNear ? 'default' : 'secondary'}>
              {pct.toFixed(0)}%
            </Badge>
          ) : (
            <Badge variant="outline">{t('cost.notSet')}</Badge>
          )}
        </CardHeader>
        <CardContent>
          {isLoading ? (
            <p className="text-sm text-muted-foreground">{t('common.loading')}</p>
          ) : limit != null ? (
            <div className="space-y-2">
              <div className="flex items-baseline justify-between">
                <span className="text-2xl font-bold">${mtdSpend.toFixed(4)}</span>
                <span className="text-sm text-muted-foreground">
                  / ${limit.toFixed(2)} {t('cost.thisMonth')}
                </span>
              </div>
              <div className="h-2.5 rounded-full bg-muted overflow-hidden">
                <div
                  className={`h-full rounded-full transition-all ${
                    isOver ? 'bg-error' : isNear ? 'bg-warning' : 'bg-primary'
                  }`}
                  style={{ width: `${pct}%` }}
                />
              </div>
              <div className="flex items-center justify-between">
                <p className="text-xs text-muted-foreground">
                  {t('cost.remaining')}: ${Math.max(0, limit - mtdSpend).toFixed(2)}
                </p>
                {(isOver || isNear) && (
                  <span className="flex items-center gap-1 text-xs text-warning">
                    <AlertTriangle className="h-3 w-3" />
                    {isOver ? t('cost.limitExceeded') : t('cost.approachingLimit')}
                  </span>
                )}
              </div>
              <Button
                size="sm"
                variant="ghost"
                className="mt-1 h-7 text-xs"
                onClick={() => setDialogOpen(true)}
              >
                <Pencil className="mr-1 h-3 w-3" />
                {t('cost.editLimit')}
              </Button>
            </div>
          ) : (
            <div className="space-y-2">
              <p className="text-sm text-muted-foreground">{t('cost.noLimitDesc')}</p>
              <Button
                size="sm"
                variant="outline"
                className="h-7 text-xs"
                onClick={() => setDialogOpen(true)}
              >
                <Target className="mr-1 h-3 w-3" />
                {t('cost.setLimit')}
              </Button>
            </div>
          )}
        </CardContent>
      </Card>

      <SetLimitDialog open={dialogOpen} onOpenChange={setDialogOpen} currentLimit={limit} />
    </>
  )
}

function SetLimitDialog({
  open,
  onOpenChange,
  currentLimit,
}: {
  open: boolean
  onOpenChange: (v: boolean) => void
  currentLimit: number | null
}) {
  const { t } = useTranslation()
  const mutation = useSetSpendLimit()
  const [value, setValue] = useState(currentLimit != null ? currentLimit.toString() : '')

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    const parsed = parseFloat(value)
    mutation.mutate(Number.isNaN(parsed) ? null : parsed, {
      onSuccess: () => onOpenChange(false),
    })
  }

  const handleClear = () => {
    mutation.mutate(null, {
      onSuccess: () => onOpenChange(false),
    })
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{t('cost.setSpendLimit')}</DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="space-y-4">
          <div className="space-y-2">
            <label className="text-sm font-medium">{t('cost.monthlyLimitUsd')}</label>
            <Input
              type="number"
              step="0.01"
              min="0"
              placeholder="50.00"
              value={value}
              onChange={(e) => setValue(e.target.value)}
            />
            <p className="text-xs text-muted-foreground">{t('cost.spendLimitHint')}</p>
          </div>
          <DialogFooter className="gap-2">
            {currentLimit != null && (
              <Button
                type="button"
                variant="ghost"
                className="text-destructive"
                onClick={handleClear}
                disabled={mutation.isPending}
              >
                {t('cost.clearLimit')}
              </Button>
            )}
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              {t('common.cancel')}
            </Button>
            <Button type="submit" disabled={mutation.isPending}>
              {t('common.save')}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
