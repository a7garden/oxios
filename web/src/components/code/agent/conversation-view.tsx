// conversation-view — scrollable message timeline for the code agent.
//
// Renders three message kinds from useCodeSessionStore.messages:
//   • user        → right-aligned bubble (primary)
//   • assistant   → left-aligned, markdown-rendered (sunken surface)
//   • system      → muted center-aligned note
//
// Tool calls attached to an assistant message render as compact,
// expandable cards below the markdown body. The list auto-scrolls to
// the bottom on new messages (unless the user has scrolled up).

import { ChevronDown, ChevronRight, Loader2, Wrench } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import ReactMarkdown from 'react-markdown'
import rehypeHighlight from 'rehype-highlight'
import remarkGfm from 'remark-gfm'
import { useCodeSessionStore } from '@/stores/code/code-session'
import type { CodeMessage, ToolCallInfo } from '@/types/code'
import { cn } from '@/lib/utils'

export interface ConversationViewProps {
  /** Optional className for the outer wrapper. */
  className?: string
}

// ── Tool call card ──────────────────────────────────────────────

interface ToolCallCardProps {
  call: ToolCallInfo
}

function ToolCallCard({ call }: ToolCallCardProps) {
  const [open, setOpen] = useState(false)
  // The args shape is opaque Record<string, unknown>; render via JSON.stringify
  // (guarded) so any value — string, number, nested object — survives.
  const argsJson = (() => {
    try {
      return JSON.stringify(call.args ?? {}, null, 2)
    } catch {
      return String(call.args)
    }
  })()

  return (
    <div className="rounded-md border border-line bg-surface text-xs">
      <button
        type="button"
        onClick={() => setOpen((v) => !v)}
        className="w-full flex items-center gap-2 px-2.5 py-1.5 text-left hover:bg-surface-sunken/60 transition"
        aria-expanded={open}
      >
        {open ? (
          <ChevronDown className="size-3.5 text-muted-foreground shrink-0" />
        ) : (
          <ChevronRight className="size-3.5 text-muted-foreground shrink-0" />
        )}
        <Wrench className="size-3.5 text-muted-foreground shrink-0" />
        <span className="font-medium truncate">{call.tool}</span>
        {call.result_summary ? (
          <span className="text-muted-foreground truncate">— {call.result_summary}</span>
        ) : null}
        {typeof call.exit_code === 'number' ? (
          <span
            className={cn(
              'ml-auto shrink-0 text-[10px] font-mono px-1.5 py-0.5 rounded',
              call.exit_code === 0
                ? 'bg-emerald-500/10 text-emerald-600 dark:text-emerald-400'
                : 'bg-destructive/10 text-destructive',
            )}
          >
            exit {call.exit_code}
          </span>
        ) : null}
      </button>
      {open ? (
        <div className="border-t border-line px-2.5 py-2 space-y-2">
          <div>
            <div className="text-[10px] uppercase tracking-wider text-muted-foreground mb-1">
              Arguments
            </div>
            <pre className="rounded bg-surface-sunken p-2 text-[11px] font-mono overflow-x-auto whitespace-pre-wrap break-words">
              {argsJson}
            </pre>
          </div>
        </div>
      ) : null}
    </div>
  )
}

// ── Message renderers ───────────────────────────────────────────

function UserBubble({ message }: { message: CodeMessage }) {
  return (
    <div className="flex justify-end">
      <div className="max-w-[85%] rounded-lg bg-primary text-primary-foreground px-3 py-2 text-sm whitespace-pre-wrap break-words shadow-sm">
        {message.content}
      </div>
    </div>
  )
}

function SystemNote({ message }: { message: CodeMessage }) {
  return (
    <div className="text-center text-xs text-muted-foreground italic px-4 py-1">
      {message.content}
    </div>
  )
}

function AssistantBubble({ message }: { message: CodeMessage }) {
  const hasToolCalls = (message.tool_calls?.length ?? 0) > 0
  return (
    <div className="flex flex-col gap-2 max-w-full">
      <div className="rounded-lg bg-surface-sunken px-3 py-2 text-sm text-foreground">
        {message.content.trim().length > 0 ? (
          <div className="prose prose-sm dark:prose-invert max-w-none break-words [&_p]:my-1 [&_ul]:my-1 [&_ol]:my-1 [&_pre]:my-1 [&_code]:font-mono">
            <ReactMarkdown remarkPlugins={[remarkGfm]} rehypePlugins={[rehypeHighlight]}>
              {message.content}
            </ReactMarkdown>
          </div>
        ) : (
          <span className="text-muted-foreground italic">…</span>
        )}
      </div>
      {hasToolCalls ? (
        <div className="flex flex-col gap-1.5 pl-1">
          {message.tool_calls!.map((call, i) => (
            <ToolCallCard key={`${message.id}-tc-${i}`} call={call} />
          ))}
        </div>
      ) : null}
      {message.model ? (
        <div className="text-[10px] text-muted-foreground pl-1">{message.model}</div>
      ) : null}
    </div>
  )
}

// ── Main view ───────────────────────────────────────────────────

/** Pixel threshold for "user is near the bottom" — within this many
 *  pixels of the scroll-bottom we treat the position as anchored. */
const BOTTOM_THRESHOLD_PX = 32

/**
 * ConversationView — the message timeline for the coding agent.
 * Subscribes to `messages`, `agentPhase`, and `isAgentRunning` from
 * the code session store. Auto-scrolls to bottom on new messages
 * unless the user has scrolled up; in that case we don't yank them.
 *
 * Uses a plain scrollable div rather than radix's ScrollArea so the
 * scroll listener and ref behave predictably.
 */
export function ConversationView({ className }: ConversationViewProps) {
  const messages = useCodeSessionStore((s) => s.messages)
  const agentPhase = useCodeSessionStore((s) => s.agentPhase)
  const isAgentRunning = useCodeSessionStore((s) => s.isAgentRunning)

  const scrollRef = useRef<HTMLDivElement | null>(null)
  // When false, the user has scrolled up and we shouldn't auto-stick.
  const stickToBottomRef = useRef(true)

  const handleScroll = () => {
    const el = scrollRef.current
    if (!el) return
    const distanceFromBottom = el.scrollHeight - el.scrollTop - el.clientHeight
    stickToBottomRef.current = distanceFromBottom <= BOTTOM_THRESHOLD_PX
  }

  // Auto-scroll to the bottom when messages arrive and the user is anchored.
  useEffect(() => {
    const el = scrollRef.current
    if (!el || !stickToBottomRef.current) return
    el.scrollTop = el.scrollHeight
  }, [messages, isAgentRunning, agentPhase])

  const empty = messages.length === 0

  return (
    <div
      ref={scrollRef}
      onScroll={handleScroll}
      className={cn('flex-1 min-h-0 overflow-y-auto', className)}
    >
      <div className="flex flex-col gap-3 px-4 py-4 min-h-full">
        {empty ? (
          <div className="flex-1 flex items-center justify-center py-12">
            <div className="text-center text-sm text-muted-foreground max-w-xs">
              <div className="text-base font-medium text-foreground mb-1">
                Ready when you are
              </div>
              Ask the agent to refactor a file, add a feature, or explain how
              something works. Tool calls and edits will appear here.
            </div>
          </div>
        ) : (
          messages.map((m) => {
            if (m.role === 'user') return <UserBubble key={m.id} message={m} />
            if (m.role === 'system') return <SystemNote key={m.id} message={m} />
            return <AssistantBubble key={m.id} message={m} />
          })
        )}

        {isAgentRunning || agentPhase ? (
          <div
            className="flex items-center gap-2 text-xs text-muted-foreground pl-1"
            aria-live="polite"
          >
            <Loader2 className="size-3 animate-spin" />
            <span>{agentPhase || 'Thinking…'}</span>
          </div>
        ) : null}
      </div>
    </div>
  )
}
