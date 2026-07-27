import { FolderPlus, ShieldCheck, X } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'

interface PathAccessCardProps {
  path: string
  mode: string
  toolName: string
  reason: string
  onMount: () => void
  onTempAllow: () => void
  onDeny: () => void
  disabled?: boolean
}

/**
 * Inline path-access card shown when an agent tries to read or write a
 * file outside its `allowed_paths`. Offers three choices:
 *   - Create Mount (persistent path alias, survives restart)
 *   - Temporarily allow (session-scoped `allowed_paths` entry)
 *   - Deny (return the error to the agent)
 *
 * Mirrors ToolApprovalCard's visual structure.
 */
export function PathAccessCard({
  path,
  mode,
  toolName,
  reason,
  onMount,
  onTempAllow,
  onDeny,
  disabled,
}: PathAccessCardProps) {
  const { i18n } = useTranslation()
  const isKo = i18n.language?.startsWith('ko')

  return (
    <div className="flex gap-3 my-1.5">
      <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-warning text-warning-foreground">
        <ShieldCheck className="h-4 w-4" />
      </div>
      <div className="max-w-[80%] flex-1">
        <div className="rounded-xl border bg-card shadow-sm">
          <div className="flex items-center gap-2 px-4 py-3 border-b">
            <ShieldCheck className="h-4 w-4 text-warning shrink-0" />
            <span className="text-sm font-medium">
              {isKo ? '경로 접근 권한' : 'Path Access'}
            </span>
            <span className="ml-auto px-2 py-0.5 rounded bg-muted text-xs font-mono">
              {mode === 'write' ? 'write' : 'read'}
            </span>
          </div>
          <div className="px-4 py-3">
            <p className="text-sm text-muted-foreground break-all font-mono text-xs">{path}</p>
            {reason && <p className="text-xs text-muted-foreground mt-2">{reason}</p>}
            <p className="text-xs text-muted-foreground mt-1">
              {isKo
                ? `${toolName} 도구가 이 경로에 접근하려고 합니다. 허용 방법을 선택하세요.`
                : `The ${toolName} tool wants to access this path. Choose how to proceed.`}
            </p>
          </div>
          <div className="flex items-center justify-end gap-2 px-4 py-3 border-t">
            <Button onClick={onDeny} variant="ghost" size="sm" disabled={disabled}>
              <X className="h-3.5 w-3.5 mr-1" />
              {isKo ? '거부' : 'Deny'}
            </Button>
            <Button onClick={onTempAllow} variant="outline" size="sm" disabled={disabled}>
              {isKo ? '임시 허용' : 'Allow once'}
            </Button>
            <Button
              onClick={onMount}
              size="sm"
              disabled={disabled}
              className="bg-success/90 hover:bg-success text-white"
            >
              <FolderPlus className="h-3.5 w-3.5 mr-1" />
              {isKo ? '마운트 생성' : 'Create Mount'}
            </Button>
          </div>
        </div>
      </div>
    </div>
  )
}
