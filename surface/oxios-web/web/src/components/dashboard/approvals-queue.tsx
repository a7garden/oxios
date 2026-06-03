import { Link } from '@tanstack/react-router'
import { AlertTriangle, Bell, CheckCircle, XCircle } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { useToast } from '@/components/ui/sonner'
import { useApproveApproval, usePendingApprovals, useRejectApproval } from '@/hooks/use-approvals'
import type { Approval } from '@/types'

/**
 * Approvals queue — full-width, shown on the dashboard.
 *
 * Each row offers inline Approve / Deny buttons that use optimistic
 * mutations. The card stays visible even when the queue is empty,
 * surfacing a positive "all clear" message so a user landing on the
 * dashboard can distinguish a healthy system from a failed query.
 * (RFC §5 originally called for hide-when-empty; the reviewer brief
 * explicitly flagged that as confusing.)
 */
export function ApprovalsQueue() {
  const { t } = useTranslation()
  const { items: pending, isLoading } = usePendingApprovals()
  const approve = useApproveApproval()
  const reject = useRejectApproval()
  const { toast } = useToast()

  const handleApprove = (id: string) => {
    approve.mutate(id, {
      onSuccess: () => toast(t('approvals.approveSuccess'), 'success'),
      onError: (err) => toast(t('approvals.mutationError', { error: String(err) }), 'destructive'),
    })
  }
  const handleDeny = (id: string) => {
    reject.mutate(id, {
      onSuccess: () => toast(t('approvals.rejectSuccess'), 'success'),
      onError: (err) => toast(t('approvals.mutationError', { error: String(err) }), 'destructive'),
    })
  }

  const empty = !isLoading && pending.length === 0

  return (
    <Card className={empty ? 'border-emerald-500/30' : 'border-amber-500/40'}>
      <CardHeader className="flex flex-row items-center justify-between space-y-0 pb-2">
        <CardTitle className="flex items-center gap-2 text-base">
          <Bell className={`h-4 w-4 ${empty ? 'text-emerald-500' : 'text-amber-500'}`} />
          {t('approvals.title')}
          <Badge variant={empty ? 'secondary' : 'warning'} className="ml-1">
            {pending.length}
          </Badge>
        </CardTitle>
        <Link
          to="/approvals"
          className="text-xs text-muted-foreground hover:text-foreground underline-offset-4 hover:underline"
        >
          {t('dashboard.viewAll')}
        </Link>
      </CardHeader>
      <CardContent className="pt-2">
        {isLoading ? (
          <p className="text-sm text-muted-foreground py-2">{t('common.loading')}</p>
        ) : empty ? (
          <div className="flex items-center gap-2 py-2 text-sm text-muted-foreground">
            <CheckCircle className="h-4 w-4 text-emerald-500" aria-hidden="true" />
            <span>{t('dashboard.approvalsAllClear')}</span>
          </div>
        ) : (
          <ul className="space-y-2">
            {pending.map((approval) => (
              <ApprovalRow
                key={approval.id}
                approval={approval}
                onApprove={handleApprove}
                onDeny={handleDeny}
                busy={approve.isPending || reject.isPending}
                approveLabel={t('approvals.approve')}
                denyLabel={t('approvals.deny')}
                riskLabel={t('dashboard.risk')}
              />
            ))}
          </ul>
        )}
      </CardContent>
    </Card>
  )
}

interface ApprovalRowProps {
  approval: Approval
  onApprove: (id: string) => void
  onDeny: (id: string) => void
  busy: boolean
  approveLabel: string
  denyLabel: string
  riskLabel: string
}

function ApprovalRow({
  approval,
  onApprove,
  onDeny,
  busy,
  approveLabel,
  denyLabel,
  riskLabel,
}: ApprovalRowProps) {
  const action = approval.action || ''
  const resource = approval.resource || ''
  const reason = approval.reason || action

  return (
    <li className="flex flex-wrap items-center gap-3 rounded-lg border bg-amber-500/5 p-3">
      <AlertTriangle className="h-4 w-4 shrink-0 text-amber-500" aria-hidden="true" />
      <div className="min-w-0 flex-1">
        <p className="text-sm font-medium text-foreground truncate" title={reason}>
          <span className="font-mono text-xs text-muted-foreground mr-1.5">{action}</span>
          {resource}
        </p>
        <p className="text-xs text-muted-foreground">
          {reason && reason !== action ? `${riskLabel}: ${reason} · ` : ''}
          {new Date(approval.created_at).toLocaleTimeString()}
        </p>
      </div>
      <div className="flex items-center gap-2 shrink-0">
        <Button
          size="sm"
          variant="outline"
          className="text-emerald-600 border-emerald-500/40 hover:bg-emerald-500/10"
          onClick={() => onApprove(approval.id)}
          disabled={busy}
          aria-label={approveLabel}
        >
          <CheckCircle className="h-3.5 w-3.5 mr-1" /> {approveLabel}
        </Button>
        <Button
          size="sm"
          variant="outline"
          className="text-red-600 border-red-500/40 hover:bg-red-500/10"
          onClick={() => onDeny(approval.id)}
          disabled={busy}
          aria-label={denyLabel}
        >
          <XCircle className="h-3.5 w-3.5 mr-1" /> {denyLabel}
        </Button>
      </div>
    </li>
  )
}
