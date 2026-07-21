// messages/components/ErrorCard — inline error display with retry.
//
// LobeHub analogue: Conversation/Messages/Error/.
// Replaces ChatItem's ErrorBlock for assistant-role errors that benefit from
// richer presentation (retry button, errorKind-specific copy + suggestion).

import { AlertTriangle, RefreshCw } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { ChatError } from '@/types/chat'

interface ErrorCardProps {
  error: ChatError
  onRetry?: () => void
  className?: string
}

interface KindCopy {
  title: string
  hint?: string
}

const KIND_COPY: Record<string, KindCopy> = {
  quota_exceeded: {
    title: 'Quota exceeded',
    hint: 'Provider rate limit or monthly quota hit. Try a different model or wait for reset.',
  },
  auth: {
    title: 'Authentication failed',
    hint: 'Check the provider API key in Settings → Providers.',
  },
  routing: {
    title: 'Routing failed',
    hint: 'No provider available for this model. Check provider config.',
  },
  unknown: {
    title: 'Something went wrong',
  },
}

const FALLBACK_COPY: KindCopy = { title: 'Something went wrong' }

export function ErrorCard({ error, onRetry, className }: ErrorCardProps) {
  const kind = (error.category ?? error.type ?? 'unknown') as string
  const copy: KindCopy = KIND_COPY[kind] ?? FALLBACK_COPY
  const severity = error.severity ?? 'error'
  const isCritical = severity === 'critical'

  return (
    <div
      className={cn(
        'rounded-md border px-3 py-2 text-sm flex items-start gap-2',
        isCritical
          ? 'border-destructive bg-destructive/10 text-destructive'
          : 'border-destructive/40 bg-destructive/5 text-destructive',
        className,
      )}
      role="alert"
    >
      <AlertTriangle className="w-4 h-4 mt-0.5 shrink-0" />
      <div className="flex-1 min-w-0">
        <div className="font-medium">{copy.title}</div>
        {error.message && <div className="text-xs mt-0.5 opacity-90">{error.message}</div>}
        {copy.hint && <div className="text-xs mt-1 opacity-75 italic">{copy.hint}</div>}
      </div>
      {onRetry && (
        <button
          type="button"
          onClick={onRetry}
          className="inline-flex items-center gap-1 px-2 py-1 rounded text-xs bg-destructive text-destructive-foreground hover:opacity-90 transition-opacity shrink-0"
        >
          <RefreshCw className="w-3 h-3" />
          Retry
        </button>
      )}
    </div>
  )
}
