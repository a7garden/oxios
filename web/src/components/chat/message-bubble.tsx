import { Bot, ClipboardList, User, Wrench } from 'lucide-react'
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
}

export function MessageBubble({ message, sessionId, assistantIndex }: MessageBubbleProps) {
  const { t } = useTranslation()
  const isUser = message.role === 'user'
  const isTool = message.role === 'tool'

  // Relative timestamp — i18n aware
  const relTime = (() => {
    if (!message.timestamp) return ''
    const diff = Date.now() - new Date(message.timestamp).getTime()
    if (diff < 60000) return t('common.justNow', 'just now')
    if (diff < 3600000) return t('common.minutesAgo', { count: Math.floor(diff / 60000) })
    if (diff < 86400000) return t('common.hoursAgo', { count: Math.floor(diff / 3600000) })
    return new Date(message.timestamp).toLocaleDateString()
  })()

  if (isTool) {
    return (
      <div className="flex gap-3 my-1">
        <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-muted">
          <span className="text-xs">
            <Wrench className="h-3.5 w-3.5" />
          </span>
        </div>
        <div className="flex-1">
          {message.toolName && (
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
          )}
        </div>
      </div>
    )
  }

  return (
    <div className={`flex gap-3 my-1.5 ${isUser ? 'justify-end' : ''}`}>
      {!isUser && (
        <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-primary text-primary-foreground">
          <Bot className="h-4 w-4" />
        </div>
      )}
      <div className="max-w-[80%]">
        <div
          className={`rounded-lg px-4 py-2 ${
            isUser ? 'bg-primary text-primary-foreground' : 'bg-muted'
          }`}
        >
          {/* Interview questions summary (persisted after submit) */}
          {!isUser && message._interviewQuestions && message._interviewQuestions.length > 0 && (
            <div className="mb-2">
              <div className="flex items-center gap-1.5 text-xs font-medium text-muted-foreground mb-1.5">
                <ClipboardList className="h-3 w-3" />
                <span>
                  Interview{message._interviewRound ? ` R${message._interviewRound}` : ''}
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
          {isUser ? (
            <p className="text-sm whitespace-pre-wrap">{message.content}</p>
          ) : message.content ? (
            <div className="text-sm prose prose-sm dark:prose-invert max-w-none [&>p:first-child]:mt-0 [&>p:last-child]:mb-0">
              <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeHighlight]}>
                {message.content}
              </ReactMarkdown>
            </div>
          ) : null}
        </div>
        {/* RFC-015: real-time activity timeline (tool calls, memory,
            reasoning, token usage). Hidden for user messages. */}
        {!isUser && message.activities && message.activities.length > 0 && (
          <ActivityTimeline activities={message.activities} />
        )}
        <div className="flex items-center gap-2 mt-1.5">
          {relTime && <span className="text-2xs text-muted-foreground">{relTime}</span>}
          {!isUser && <ChatMetadata message={message} />}
        </div>
        {/* RFC-016: Knowledge save toggle — assistant messages only */}
        {!isUser && sessionId && assistantIndex !== undefined && (
          <KnowledgeSaveIndicator sessionId={sessionId} messageIndex={assistantIndex} />
        )}
      </div>
      {isUser && (
        <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-muted">
          <User className="h-4 w-4" />
        </div>
      )}
    </div>
  )
}
