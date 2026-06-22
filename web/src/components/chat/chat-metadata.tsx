import { CheckCircle2, Clock, XCircle } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { ChatMessage } from '@/types'

interface ChatMetadataProps {
  message: ChatMessage
  className?: string
}

export function ChatMetadata({ message, className }: ChatMetadataProps) {
  if (!message.metadata) return null

  const { phase, evaluation_passed, duration_ms } = message.metadata
  const durationStr = duration_ms
    ? duration_ms >= 60000
      ? `${Math.floor(duration_ms / 60000)}m ${Math.round((duration_ms % 60000) / 1000)}s`
      : duration_ms >= 1000
        ? `${(duration_ms / 1000).toFixed(1)}s`
        : `${duration_ms}ms`
    : null

  return (
    <div
      className={cn(
        'flex items-center gap-2 text-xs text-muted-foreground mt-1 flex-wrap',
        className,
      )}
    >
      {phase && <span className="px-1.5 py-0.5 rounded bg-muted font-medium">{phase}</span>}
      {evaluation_passed === true && (
        <span className="flex items-center gap-1 text-success">
          <CheckCircle2 className="h-3.5 w-3.5" /> Passed
        </span>
      )}
      {evaluation_passed === false && phase !== 'interview' && (
        <span className="flex items-center gap-1 text-error">
          <XCircle className="h-3.5 w-3.5" /> Failed
        </span>
      )}
      {durationStr && (
        <span className="flex items-center gap-1">
          <Clock className="h-3 w-3" /> {durationStr}
        </span>
      )}
    </div>
  )
}
