import { AlertCircle, Bot, ClipboardList, KeyRound, RefreshCw, Route } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import ReactMarkdown from 'react-markdown'
import rehypeHighlight from 'rehype-highlight'
import remarkGfm from 'remark-gfm'
import type { ChatMessage } from '@/types'
import { ActivityTimeline } from './activity-timeline'
import { ChatMetadata } from './chat-metadata'
import { KnowledgeSaveIndicator } from './knowledge-save-indicator'
import { ToolCallCard } from './tool-call-card'

interface MessageBubbleProps {
  message: ChatMessage
  /** Session ID for knowledge save tracking (RFC-016). */
  sessionId?: string
  /** Index of this message among assistant messages only (RFC-016). */
  assistantIndex?: number
  /** RFC-032: retry the last failed send. Called from the inline error card. */
  onRetry?: () => void
}

/**
 * Chat message renderer — restrained, Claude-inspired layout.
 *
 * - User messages are subtle muted cards (left-aligned, NOT right-aligned
 *   bubbles): a small uppercase role label + the prompt. They never squeeze
 *   the assistant's rich output.
 * - Assistant messages are background-less, full-width markdown prose. A
 *   small "oxios" role mark precedes them. The activity timeline (reasoning,
 *   tool calls, usage) renders ABOVE the prose so the trace is the preamble
 *   and the answer is the hero — then metadata + the knowledge-save affordance.
 * - Tool messages (`role === 'tool'`) render as a full-width `ToolCallCard`.
 * - Inline error cards (RFC-032) keep the Oxios error-subtle treatment.
 *
 * No chat bubbles; the narrow-`bg-muted`-on-code wart is gone.
 */
export function MessageBubble({ message, sessionId, assistantIndex, onRetry }: MessageBubbleProps) {
  const { t, i18n } = useTranslation()
  const isUser = message.role === 'user'
  const isTool = message.role === 'tool'

  // Timestamp — absolute time with hour:minute (today → HH:MM, else M/D HH:MM)
  const relTime = (() => {
    if (!message.timestamp) return ''
    const d = new Date(message.timestamp)
    if (Date.now() - d.getTime() < 60000) return t('common.justNow', 'just now')
    const hm = d.toLocaleTimeString(i18n.language, { hour: '2-digit', minute: '2-digit' })
    if (d.toDateString() === new Date().toDateString()) return hm
    return `${d.toLocaleDateString(i18n.language, { month: 'numeric', day: 'numeric' })} ${hm}`
  })()
  // Model mark — strip the `provider/` prefix for the visible chip; fall back
  // gracefully when the turn has no model (e.g. pre-threaded history).
  const modelMark = message.model
    ? message.model.includes('/')
      ? message.model.split('/').slice(1).join('/')
      : message.model
    : null

  // ── tool messages: full-width ToolCallCard, no bubble ──────────────
  if (isTool) {
    if (!message.toolName) return null
    return (
      <div className="my-1.5">
        <ToolCallCard
          call={{
            tool_name: message.toolName,
            input:
              typeof message.toolArgs === 'string'
                ? message.toolArgs
                : JSON.stringify(message.toolArgs ?? '', null, 2),
            output:
              typeof message.toolResult === 'string'
                ? message.toolResult
                : JSON.stringify(message.toolResult ?? '', null, 2),
            duration_ms: message.toolDurationMs ?? 0,
          }}
        />
      </div>
    )
  }

  // ── user messages: subtle muted card, left-aligned ─────────────────
  if (isUser) {
    return (
      <div className="my-3">
        <div className="rounded-lg border border-border bg-muted px-4 py-3">
          <p className="m-0 text-sm leading-relaxed whitespace-pre-wrap">{message.content}</p>
        </div>
      </div>
    )
  }

  // ── assistant messages: background-less, full-width ────────────────
  return (
    <div className="my-3">
      {/* Model mark — prominent chip above the answer */}
      {modelMark && (
        <div className="mb-3 flex items-center gap-1.5">
          <span className="inline-flex items-center gap-1 rounded-full bg-primary/10 border border-primary/20 px-2.5 py-0.5 text-xs font-medium text-primary">
            <Bot className="h-3 w-3" />
            {modelMark}
          </span>
        </div>
      )}
      {/* Interview questions summary (persisted after submit) */}
      {message._interviewQuestions && message._interviewQuestions.length > 0 && (
        <div className="mb-3 rounded-lg border border-border bg-muted/40 p-3">
          <div className="mb-1.5 flex items-center gap-1.5 text-xs font-medium text-muted-foreground">
            <ClipboardList className="h-3 w-3" />
            <span>
              {t('chat.interviewTitle', 'Interview')}
              {message._interviewRound ? ` R${message._interviewRound}` : ''}
            </span>
          </div>
          <div className="space-y-1">
            {message._interviewQuestions.map((q, i) => (
              <p key={q.id} className="text-xs text-muted-foreground">
                {i + 1}. {q.text}
              </p>
            ))}
          </div>
        </div>
      )}

      {/* RFC-032: inline error card */}
      {message.metadata?.isError ? (
        <div className="mb-2 flex items-start gap-2.5 rounded-lg border border-error/30 bg-error/5 px-3.5 py-3 text-sm">
          {message.metadata.errorKind === 'auth' ? (
            <KeyRound className="mt-0.5 h-4 w-4 shrink-0 text-error" />
          ) : message.metadata.errorKind === 'routing' ? (
            <Route className="mt-0.5 h-4 w-4 shrink-0 text-error" />
          ) : (
            <AlertCircle className="mt-0.5 h-4 w-4 shrink-0 text-error" />
          )}
          <div className="min-w-0 flex-1">
            <p className="font-medium text-error">
              {message.metadata.errorKind === 'quota_exceeded'
                ? t('chat.error.quotaExceeded', '선택한 프로바이더에 토큰이 남아있지 않습니다.')
                : message.metadata.errorKind === 'auth'
                  ? t('chat.error.authFailed', '프로바이더 인증에 실패했습니다.')
                  : message.metadata.errorKind === 'routing'
                    ? t('chat.error.noRoute', '라우팅 가능한 프로바이더가 없습니다.')
                    : t('chat.error.generateFailed', '응답을 생성하지 못했습니다.')}
            </p>
            {message.content && (
              <p className="mt-1 whitespace-pre-wrap text-xs text-muted-foreground">
                {message.content}
              </p>
            )}
            {onRetry && (
              <button
                type="button"
                onClick={onRetry}
                className="mt-2.5 inline-flex items-center gap-1.5 rounded-md border border-error/30 bg-background px-2.5 py-1 text-xs font-medium text-error transition-colors hover:bg-error/10 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
              >
                <RefreshCw className="h-3 w-3" />
                {t('chat.retry', '다시 시도')}
              </button>
            )}
          </div>
        </div>
      ) : (
        <>
          {/* RFC-015: activity timeline (reasoning / tool calls / usage) —
              renders ABOVE the prose so the trace is the preamble and the
              answer is the hero. */}
          {message.activities && message.activities.length > 0 && (
            <div className="mb-2">
              <ActivityTimeline activities={message.activities} />
            </div>
          )}

          {/* Markdown body — full-width, no background bubble */}
          {message.content && (
            <div className="max-w-none text-sm prose prose-sm dark:prose-invert [&>p:first-child]:mt-0 [&>p:last-child]:mb-0">
              <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeHighlight]}>
                {message.content}
              </ReactMarkdown>
            </div>
          )}
        </>
      )}

      {/* Footer: timestamp + phase/eval/duration + tokens */}
      <div className="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 border-t border-border pt-2 text-2xs text-muted-foreground">
        {relTime && <span>{relTime}</span>}
        <ChatMetadata message={message} className="mt-0" />
        {(message.totalInputTokens || message.totalOutputTokens) && (
          <span>
            in {message.totalInputTokens ?? 0} · out {message.totalOutputTokens ?? 0} tok
          </span>
        )}
      </div>

      {/* RFC-016: Knowledge save — assistant messages only */}
      {sessionId && assistantIndex !== undefined && (
        <KnowledgeSaveIndicator sessionId={sessionId} messageIndex={assistantIndex} />
      )}
    </div>
  )
}
