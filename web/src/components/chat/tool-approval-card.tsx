import { ShieldAlert } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'

interface ToolApprovalCardProps {
  toolName: string
  reason: string
  onApprove: () => void
  onDeny: () => void
  disabled?: boolean
}

/**
 * Inline tool approval card shown in the chat when an agent tries
 * a tool it doesn't have CSpace capability for (RFC-017).
 */
export function ToolApprovalCard({
  toolName,
  reason,
  onApprove,
  onDeny,
  disabled,
}: ToolApprovalCardProps) {
  const { t } = useTranslation()

  return (
    <div className="flex gap-3 my-1.5">
      <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-warning text-warning-foreground">
        <ShieldAlert className="h-4 w-4" />
      </div>
      <div className="max-w-[80%] flex-1">
        <div className="rounded-xl border bg-card shadow-sm">
          <div className="flex items-center gap-2 px-4 py-3 border-b">
            <ShieldAlert className="h-4 w-4 text-warning shrink-0" />
            <span className="text-sm font-medium">{t('chat.toolApproval.title')}</span>
            <span className="ml-auto px-2 py-0.5 rounded bg-muted text-xs font-mono">
              {toolName}
            </span>
          </div>
          <div className="px-4 py-3">
            <p className="text-sm text-muted-foreground">{reason}</p>
            <p className="text-xs text-muted-foreground mt-2">
              {t('chat.toolApproval.description')}
            </p>
          </div>
          <div className="flex justify-end gap-2 px-4 py-3 border-t">
            <Button onClick={onDeny} variant="ghost" size="sm" disabled={disabled}>
              {t('chat.toolApproval.deny')}
            </Button>
            <Button
              onClick={onApprove}
              size="sm"
              disabled={disabled}
              className="bg-success/90 hover:bg-success text-white"
            >
              {t('chat.toolApproval.approve')} ✅
            </Button>
          </div>
        </div>
      </div>
    </div>
  )
}
