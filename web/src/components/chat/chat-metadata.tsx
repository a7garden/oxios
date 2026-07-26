import { CheckCircle2, XCircle } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { ChatMessage } from '@/types'

interface ChatMetadataProps {
  message: ChatMessage
  className?: string
}

/**
 * Post-turn review outcome for an assistant message.
 *
 * Only the assess→crystallize→execute→**review** gate result (Passed ✓ /
 * Failed ✗) is rendered here, and only when a review actually ran — for a
 * normal chat there is none, so this component renders nothing.
 *
 * The Ouroboros `phase` badge (almost always "execute") and the `duration_ms`
 * clock used to live in this always-visible row; both were noise on every
 * message. The phase is already surfaced live during the turn by the
 * LiveActivityBar holder, and the duration moved to ChatItem's hover
 * TitleRow next to the timestamp.
 */
export function ChatMetadata({ message, className }: ChatMetadataProps) {
  if (!message.metadata) return null
  const { phase, evaluation_passed } = message.metadata

  const passed = evaluation_passed === true
  // Suppress the Failed badge during the interview phase (not a real failure).
  const failed = evaluation_passed === false && phase !== 'interview'
  if (!passed && !failed) return null

  return (
    <div className={cn('flex items-center gap-2 text-xs mt-1 flex-wrap', className)}>
      {passed && (
        <span className="flex items-center gap-1 text-success">
          <CheckCircle2 className="h-3.5 w-3.5" /> Passed
        </span>
      )}
      {failed && (
        <span className="flex items-center gap-1 text-error">
          <XCircle className="h-3.5 w-3.5" /> Failed
        </span>
      )}
    </div>
  )
}
