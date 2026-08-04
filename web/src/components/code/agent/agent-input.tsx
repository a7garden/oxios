// agent-input — auto-resizing textarea + model selector + send/stop button.
//
// Sends messages via the streaming WebSocket (useCodeStream), not a
// blocking POST. The user message is optimistically appended to the
// session store; the streaming hook creates the assistant message and
// accumulates tokens in real-time.

import { AlertCircle, Loader2, Send, Square } from 'lucide-react'
import {
  type ChangeEvent,
  type KeyboardEvent,
  useCallback,
  useEffect,
  useRef,
  useState,
} from 'react'
import { Button } from '@/components/ui/button'
import { useCodeSessionStore } from '@/stores/code/code-session'
import { ModelSelector } from './model-selector'

export interface AgentInputProps {
  /** Optional className for the outer wrapper. */
  className?: string
  /** When true, the input is collapsed / disabled. */
  disabled?: boolean
  /** Send handler from useCodeStream — returns false if WS not ready. */
  onSend: (content: string, model?: string | null) => boolean
  /** Stop handler from useCodeStream. */
  onStop: () => void
}

/** Minimum rows for the textarea — keeps the bar present even when empty. */
const MIN_ROWS = 1
/** Cap auto-resize at this many lines before the textarea scrolls. */
const MAX_ROWS = 8

/**
 * Compute the textarea `rows` attribute by line count, clamped to
 * [MIN_ROWS, MAX_ROWS]. The textarea itself scrolls once MAX_ROWS is hit.
 */
function rowsFor(text: string): number {
  const lines = text.split('\n').length
  return Math.min(MAX_ROWS, Math.max(MIN_ROWS, lines))
}

/**
 * AgentInput — the bottom of the right-side agent panel. Combines a
 * model selector, auto-resizing textarea, and a send/stop button.
 *
 * Behaviour:
 *   • Enter sends, Shift+Enter inserts a newline.
 *   • Cmd+Enter (or Ctrl+Enter) also sends.
 *   • While the agent is running, the send button becomes a stop button.
 */
export function AgentInput({ className, disabled = false, onSend, onStop }: AgentInputProps) {
  const session = useCodeSessionStore((s) => s.session)
  const isAgentRunning = useCodeSessionStore((s) => s.isAgentRunning)
  const addMessage = useCodeSessionStore((s) => s.addMessage)

  const [value, setValue] = useState('')
  const [model, setModel] = useState<string | null>(session?.model ?? null)
  const [error, setError] = useState<string | null>(null)
  const taRef = useRef<HTMLTextAreaElement | null>(null)

  // Sync the local model when the session changes (initial load or switch).
  useEffect(() => {
    setModel(session?.model ?? null)
  }, [session?.id, session?.model])

  const onChange = useCallback((e: ChangeEvent<HTMLTextAreaElement>) => {
    setValue(e.target.value)
  }, [])

  const send = useCallback(() => {
    const text = value.trim()
    if (!text || !session) return
    setError(null)

    // Optimistic user message.
    addMessage({
      id: `local-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`,
      role: 'user',
      content: text,
      timestamp: new Date().toISOString(),
      model: model ?? undefined,
    })
    setValue('')

    const ok = onSend(text, model)
    if (!ok) {
      setError('Not connected — try again in a moment.')
    }
  }, [value, session, model, addMessage, onSend])

  const stop = useCallback(() => {
    onStop()
  }, [onStop])

  const onKeyDown = useCallback(
    (e: KeyboardEvent<HTMLTextAreaElement>) => {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault()
        if (!isAgentRunning && !disabled) void send()
        return
      }
      if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
        e.preventDefault()
        if (!isAgentRunning && !disabled) void send()
        return
      }
      if (e.key === 'Escape' && isAgentRunning) {
        e.preventDefault()
        stop()
      }
    },
    [send, stop, isAgentRunning, disabled],
  )

  const trimmed = value.trim()
  const canSend = trimmed.length > 0 && !!session && !isAgentRunning && !disabled

  return (
    <div
      className={`border-t border-line bg-surface px-3 py-3 flex flex-col gap-2 ${className ?? ''}`}
    >
      {error ? (
        <div className="flex items-center gap-1.5 text-xs text-destructive">
          <AlertCircle className="size-3.5" />
          <span className="truncate">{error}</span>
        </div>
      ) : null}
      <div className="rounded-lg border border-line bg-background focus-within:ring-1 focus-within:ring-ring/40 focus-within:border-primary/40 transition">
        <textarea
          ref={taRef}
          value={value}
          onChange={onChange}
          onKeyDown={onKeyDown}
          rows={rowsFor(value)}
          placeholder={
            session
              ? 'Ask the agent to make changes, explain code, or run tasks…'
              : 'Start or select a session to begin.'
          }
          disabled={disabled || !session}
          className="w-full resize-none bg-transparent px-3 pt-2.5 pb-1 text-sm leading-relaxed placeholder:text-muted-foreground focus:outline-none disabled:opacity-50"
        />
        <div className="flex items-center gap-1 px-1.5 pb-1.5">
          <ModelSelector
            value={model}
            onChange={setModel}
            disabled={disabled || !session}
            className="h-7"
          />
          <div className="ml-auto flex items-center gap-1.5">
            <span className="text-[10px] text-muted-foreground hidden sm:inline">
              {navigator?.platform?.includes('Mac') ? '⌘' : 'Ctrl'}+↵ to send
            </span>
            {isAgentRunning ? (
              <Button
                type="button"
                size="sm"
                variant="destructive"
                onClick={stop}
                className="h-7 px-2.5 text-xs"
                aria-label="Stop agent"
              >
                <Square className="size-3.5" fill="currentColor" />
                <span>Stop</span>
              </Button>
            ) : (
              <Button
                type="button"
                size="sm"
                onClick={() => void send()}
                disabled={!canSend}
                className="h-7 px-2.5 text-xs"
                aria-label="Send message"
              >
                {isAgentRunning ? (
                  <Loader2 className="size-3.5 animate-spin" />
                ) : (
                  <Send className="size-3.5" />
                )}
                <span>Send</span>
              </Button>
            )}
          </div>
        </div>
      </div>
    </div>
  )
}
