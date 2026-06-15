import { BookOpen, Brain, FileText, Send, Square, X } from 'lucide-react'
import { type KeyboardEvent, useCallback, useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { useIsTouch } from '@/hooks/use-is-touch'
import { useKnowledgeSearch } from '@/hooks/use-knowledge'
import { useMemorySemanticSearch } from '@/hooks/use-memory'
import { cn } from '@/lib/utils'

// ── Context item attached via @mention ────────────────────────

export interface ContextAttachment {
  type: 'knowledge' | 'memory'
  id: string
  label: string
  /** Short snippet for preview */
  snippet?: string
}

// ── Unified search result for the popover ─────────────────────

interface MentionResult {
  type: 'knowledge' | 'memory'
  id: string
  label: string
  snippet: string
  score?: number
}

// ── Props ─────────────────────────────────────────────────────

interface ChatInputProps {
  value: string
  onChange: (value: string) => void
  onSend: (content: string, contextItems: ContextAttachment[]) => void
  onCancel?: () => void
  disabled?: boolean
  isStreaming?: boolean
  connected?: boolean
  /** Whether spec (Ouroboros) mode is active for this session. */
  specMode?: boolean
}

// ── Component ─────────────────────────────────────────────────

/**
 * Claude-inspired chat input with auto-growing textarea and @mention popover.
 *
 * - Auto-grows 1 → 10 lines
 * - Shift+Enter for new line, Enter to send
 * - @ triggers context search (knowledge base + memory)
 * - Mode badge (read-only) shown when session is active
 */
export function ChatInput({
  value,
  onChange,
  onSend,
  onCancel,
  disabled,
  isStreaming,
  connected,
  specMode,
}: ChatInputProps) {
  const { t } = useTranslation()
  const textareaRef = useRef<HTMLTextAreaElement>(null)
  const [isComposing, setIsComposing] = useState(false)
  const isTouch = useIsTouch()

  // ── @mention state ──
  const [mentionQuery, setMentionQuery] = useState<string | null>(null)
  const [mentionIndex, setMentionIndex] = useState(0)
  const [contextAttachments, setContextAttachments] = useState<ContextAttachment[]>([])
  const [mentionResults, setMentionResults] = useState<MentionResult[]>([])
  const mentionSearchTimer = useRef<ReturnType<typeof setTimeout> | null>(null)

  const knowledgeSearch = useKnowledgeSearch()
  const memorySearch = useMemorySemanticSearch()

  // ── Auto-grow ──
  const adjustHeight = useCallback(() => {
    const el = textareaRef.current
    if (!el) return
    el.style.height = 'auto'
    const lineHeight = parseInt(getComputedStyle(el).lineHeight, 10) || 24
    const maxHeight = lineHeight * 10
    el.style.height = `${Math.min(el.scrollHeight, maxHeight)}px`
  }, [])

  useEffect(() => {
    adjustHeight()
  }, [value, adjustHeight])

  // Focus on mount
  useEffect(() => {
    if (connected && !disabled) textareaRef.current?.focus()
  }, [connected, disabled])

  // ── @mention search ──
  useEffect(() => {
    if (mentionQuery === null) {
      setMentionResults([])
      return
    }

    // Debounce search
    if (mentionSearchTimer.current) clearTimeout(mentionSearchTimer.current)
    mentionSearchTimer.current = setTimeout(async () => {
      const results: MentionResult[] = []

      // Search knowledge base
      try {
        const kRes = await knowledgeSearch.mutateAsync({ query: mentionQuery, limit: 5 })
        for (const hit of kRes.results) {
          results.push({
            type: 'knowledge',
            id: hit.path,
            label: hit.name,
            snippet: hit.snippet.slice(0, 80),
          })
        }
      } catch {
        // Knowledge search not available
      }

      // Search memory
      try {
        const mRes = await memorySearch.mutateAsync({ query: mentionQuery, limit: 5 })
        for (const entry of mRes.entries) {
          results.push({
            type: 'memory',
            id: entry.id,
            label: entry.key || entry.id.slice(0, 12),
            snippet: (entry.summary || entry.content).slice(0, 80),
            score: entry.score,
          })
        }
      } catch {
        // Memory search not available
      }

      // Sort: knowledge first, then by score
      results.sort((a, b) => {
        if (a.type !== b.type) return a.type === 'knowledge' ? -1 : 1
        return (b.score ?? 0) - (a.score ?? 0)
      })

      setMentionResults(results.slice(0, 8))
      setMentionIndex(0)
    }, 200)

    return () => {
      if (mentionSearchTimer.current) clearTimeout(mentionSearchTimer.current)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mentionQuery])

  // ── Insert mention ──
  const insertMention = useCallback(
    (result: MentionResult) => {
      const textarea = textareaRef.current
      if (!textarea) return

      // Find the @ that started this mention
      const cursorPos = textarea.selectionStart
      const textBeforeCursor = value.slice(0, cursorPos)
      const atIndex = textBeforeCursor.lastIndexOf('@')
      if (atIndex === -1) return

      const before = value.slice(0, atIndex)
      const after = value.slice(cursorPos)

      // Insert mention token
      const mentionToken = `@${result.label} `
      const newValue = `${before}${mentionToken}${after}`
      onChange(newValue)

      // Add to context attachments (dedup by id)
      setContextAttachments((prev) => {
        if (prev.some((a) => a.id === result.id && a.type === result.type)) return prev
        return [
          ...prev,
          { type: result.type, id: result.id, label: result.label, snippet: result.snippet },
        ]
      })

      setMentionQuery(null)
      setMentionResults([])

      // Refocus
      requestAnimationFrame(() => {
        const newPos = before.length + mentionToken.length
        textarea.setSelectionRange(newPos, newPos)
        textarea.focus()
      })
    },
    [value, onChange],
  )

  // ── Remove attachment ──
  const removeAttachment = useCallback((id: string) => {
    setContextAttachments((prev) => prev.filter((a) => a.id !== id))
  }, [])

  // ── Keyboard handling ──
  const handleKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    // Mention popover navigation
    if (mentionQuery !== null && mentionResults.length > 0) {
      if (e.key === 'ArrowDown') {
        e.preventDefault()
        setMentionIndex((i) => (i + 1) % mentionResults.length)
        return
      }
      if (e.key === 'ArrowUp') {
        e.preventDefault()
        setMentionIndex((i) => (i - 1 + mentionResults.length) % mentionResults.length)
        return
      }
      if (e.key === 'Enter' || e.key === 'Tab') {
        e.preventDefault()
        insertMention(mentionResults[mentionIndex]!)
        return
      }
      if (e.key === 'Escape') {
        e.preventDefault()
        setMentionQuery(null)
        setMentionResults([])
        return
      }
    }

    if (isComposing) return

    if (e.key === 'Enter' && !e.shiftKey) {
      if (!isTouch) {
        e.preventDefault()
        handleSend()
      }
    }
  }

  // ── Text change with @ detection ──
  const handleChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const newValue = e.target.value
    onChange(newValue)

    // Detect @mention
    const cursorPos = e.target.selectionStart
    const textBeforeCursor = newValue.slice(0, cursorPos)
    const match = textBeforeCursor.match(/@(\S*)$/)

    if (match) {
      setMentionQuery(match[1] || '')
    } else {
      if (mentionQuery !== null) {
        setMentionQuery(null)
        setMentionResults([])
      }
    }
  }

  // ── Send ──
  const handleSend = useCallback(() => {
    if (!value.trim() || isStreaming || !connected) return
    onSend(value.trim(), contextAttachments)
    onChange('')
    setContextAttachments([])
    setMentionQuery(null)
    setMentionResults([])
  }, [value, isStreaming, connected, contextAttachments, onSend, onChange])

  const canSend = value.trim() && !isStreaming && connected

  return (
    <div className="w-full max-w-3xl mx-auto px-4 pb-4 pt-2 relative">
      {/* ── @mention Popover ── */}
      {mentionQuery !== null && mentionResults.length > 0 && (
        <div className="absolute bottom-full left-4 right-4 mb-1 z-50 max-h-64 overflow-y-auto rounded-xl border bg-popover shadow-lg">
          <div className="p-1.5">
            {mentionResults.map((result, i) => (
              <button
                key={`${result.type}-${result.id}`}
                type="button"
                onClick={() => insertMention(result)}
                className={cn(
                  'flex items-start gap-2.5 w-full rounded-lg px-2.5 py-2 text-left transition-colors',
                  i === mentionIndex ? 'bg-accent text-accent-foreground' : 'hover:bg-accent/50',
                )}
              >
                {result.type === 'knowledge' ? (
                  <FileText className="h-4 w-4 mt-0.5 shrink-0 text-blue-500" />
                ) : (
                  <Brain className="h-4 w-4 mt-0.5 shrink-0 text-purple-500" />
                )}
                <div className="min-w-0 flex-1">
                  <p className="text-sm font-medium truncate">{result.label}</p>
                  {result.snippet && (
                    <p className="text-xs text-muted-foreground truncate">{result.snippet}</p>
                  )}
                </div>
                <span className="text-2xs text-muted-foreground/60 shrink-0 mt-0.5">
                  {result.type === 'knowledge' ? 'KB' : 'Memory'}
                </span>
              </button>
            ))}
          </div>
        </div>
      )}

      {/* ── Active context attachments (chips) ── */}
      {contextAttachments.length > 0 && (
        <div className="flex flex-wrap gap-1.5 mb-2">
          {contextAttachments.map((ctx) => (
            <span
              key={`${ctx.type}-${ctx.id}`}
              className="inline-flex items-center gap-1 rounded-lg bg-muted/80 px-2 py-0.5 text-xs text-foreground"
            >
              {ctx.type === 'knowledge' ? (
                <BookOpen className="h-3 w-3 text-blue-500" />
              ) : (
                <Brain className="h-3 w-3 text-purple-500" />
              )}
              <span className="truncate max-w-[120px]">{ctx.label}</span>
              <button
                type="button"
                onClick={() => removeAttachment(ctx.id)}
                className="ml-0.5 rounded-full p-0.5 hover:bg-muted-foreground/20 transition-colors"
              >
                <X className="h-2.5 w-2.5" />
              </button>
            </span>
          ))}
        </div>
      )}

      {/* ── Input container ── */}
      <div
        className={cn(
          'relative rounded-2xl border bg-background shadow-sm transition-all',
          'focus-within:shadow-md focus-within:border-primary/40 focus-within:ring-1 focus-within:ring-ring/30',
          specMode && 'border-primary/30',
          !connected && 'opacity-60',
          isStreaming && 'border-destructive/30',
        )}
      >
        <textarea
          ref={textareaRef}
          value={value}
          onChange={handleChange}
          onKeyDown={handleKeyDown}
          onCompositionStart={() => setIsComposing(true)}
          onCompositionEnd={() => setIsComposing(false)}
          placeholder={
            connected
              ? specMode
                ? t('chat.inputPlaceholderSpec', 'Describe your task in detail...')
                : t('chat.inputPlaceholder', 'Type a message... (@ to add context)')
              : t('chat.waitingForConnection', 'Waiting for connection...')
          }
          disabled={disabled || !connected}
          rows={1}
          className={cn(
            'w-full resize-none bg-transparent px-4 pt-3.5 pb-10 text-sm',
            'placeholder:text-muted-foreground/70',
            'focus:outline-none disabled:cursor-not-allowed',
            'max-h-[280px] overflow-y-auto',
          )}
        />

        {/* ── Bottom bar ── */}
        <div className="absolute bottom-0 left-0 right-0 flex items-center justify-end px-2 pb-2 pt-1">
          {/* Right: send / stop */}
          <div className="flex items-center">
            {isStreaming ? (
              <Button
                onClick={onCancel}
                variant="destructive"
                size="sm"
                className="h-8 rounded-lg px-3 text-xs gap-1.5"
                aria-label={t('chat.cancel', 'Cancel')}
              >
                <Square className="h-3 w-3 fill-current" />
                {t('chat.stop', 'Stop')}
              </Button>
            ) : (
              <Button
                onClick={handleSend}
                disabled={!canSend}
                size="icon"
                className={cn(
                  'h-11 w-11 rounded-lg transition-all sm:h-9 sm:w-9',
                  canSend
                    ? 'bg-primary text-primary-foreground hover:bg-primary/90'
                    : 'bg-muted text-muted-foreground',
                )}
                aria-label={t('common.sendMessage', 'Send')}
              >
                <Send className="h-4 w-4" />
              </Button>
            )}
          </div>
        </div>
      </div>

      {/* ── Hint ── */}
      <p className="mt-1.5 text-center text-2xs text-muted-foreground/70 hidden sm:block">
        <kbd className="rounded border bg-muted/60 px-1.5 py-0.5 text-2xs font-mono text-muted-foreground">
          Enter
        </kbd>
        {' send · '}
        <kbd className="rounded border bg-muted/60 px-1.5 py-0.5 text-2xs font-mono text-muted-foreground">
          Shift+Enter
        </kbd>
        {' new line · '}
        <kbd className="rounded border bg-muted/60 px-1.5 py-0.5 text-2xs font-mono text-muted-foreground">
          ⌘⇧M
        </kbd>
        {' mode · '}
        <kbd className="rounded border bg-muted/60 px-1.5 py-0.5 text-2xs font-mono text-muted-foreground">
          ⌘⇧N
        </kbd>
        {' new chat'}
      </p>
    </div>
  )
}
