import { BookOpen, Brain, Clock, FileText, HardDrive, Send, Square, X } from 'lucide-react'
import { type KeyboardEvent, useCallback, useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { useIsTouch } from '@/hooks/use-is-touch'
import { useKnowledgeSearch } from '@/hooks/use-knowledge'
import { useMemorySemanticSearch } from '@/hooks/use-memory'
import { useMounts } from '@/hooks/use-mounts'
import { cn } from '@/lib/utils'
import { ModelPickerContainer } from './model-picker'

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
  type: 'mount' | 'knowledge' | 'memory'
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
  /** Number of user messages queued behind the in-flight turn. */
  queuedCount?: number
  /** RFC-032: available roles (role name + model ID). */
  roles?: { name: string; model: string }[]
  /** RFC-032: currently active role (null = default). */
  activeRole?: string | null
  /** RFC-032: setter for active role. */
  setActiveRole?: (role: string | null) => void
  /** Per-message model override id (null = no override). */
  activeModelId?: string | null
  setActiveModelId?: (id: string | null) => void
  /** RFC-025: mounts bound to the active session (session-sticky chips). */
  activeMounts?: { id: string; label: string }[]
  /** RFC-025: bind a Mount to the active session (@mount or drag-drop). */
  onAttachMount?: (id: string) => void
  /** RFC-025: unbind a Mount from the active session. */
  onRemoveMount?: (id: string) => void
}
// ── Component ─────────────────────────────────────────────────

/**
 * Claude-inspired chat input with auto-growing textarea and @mention popover.
 *
 * - Auto-grows 1 → 10 lines
 * - Shift+Enter for new line, Enter to send
 * - @ triggers context search (knowledge base + memory)
 */
export function ChatInput({
  value,
  onChange,
  onSend,
  onCancel,
  disabled,
  isStreaming,
  connected,
  queuedCount = 0,
  roles = [],
  activeRole = null,
  setActiveRole = () => {},
  activeModelId = null,
  setActiveModelId = () => {},
  activeMounts = [],
  onAttachMount = () => {},
  onRemoveMount = () => {},
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
  const { data: mountsData } = useMounts()

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

      // Search mounts (client-side filter — mounts are few). RFC-025: a Mount
      // is the addressable filesystem concept; @mount binds it to the session
      // (path access + CWD + workspace context), NOT a per-message text ref.
      const mq = mentionQuery.toLowerCase()
      for (const m of mountsData?.items ?? []) {
        if (
          m.name.toLowerCase().includes(mq) ||
          m.auto_description.toLowerCase().includes(mq) ||
          m.paths.some((p) => p.toLowerCase().includes(mq))
        ) {
          results.push({
            type: 'mount',
            id: m.id,
            label: m.name,
            snippet: m.auto_description.slice(0, 80),
          })
        }
      }

      // Sort: mounts first (heaviest/most intentional), then knowledge, then
      // memory, with semantic score breaking ties within a kind.
      const kindRank = (t: MentionResult['type']) => (t === 'mount' ? 0 : t === 'knowledge' ? 1 : 2)
      results.sort((a, b) => {
        if (a.type !== b.type) return kindRank(a.type) - kindRank(b.type)
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

      if (result.type === 'mount') {
        // RFC-025: @mount binds to the session (path access + CWD + workspace
        // context), not a per-message text ref. Route to the session binding.
        onAttachMount(result.id)
      } else {
        // Narrow: in this branch result is knowledge | memory, not mount.
        const nonMount: ContextAttachment = {
          type: result.type,
          id: result.id,
          label: result.label,
          snippet: result.snippet,
        }
        setContextAttachments((prev) =>
          prev.some((a) => a.id === nonMount.id && a.type === nonMount.type)
            ? prev
            : [...prev, nonMount],
        )
      }

      setMentionQuery(null)
      setMentionResults([])

      // Refocus
      requestAnimationFrame(() => {
        const newPos = before.length + mentionToken.length
        textarea.setSelectionRange(newPos, newPos)
        textarea.focus()
      })
    },
    [value, onChange, onAttachMount],
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
    if (!value.trim() || !connected) return
    onSend(value.trim(), contextAttachments)
    onChange('')
    setContextAttachments([])
    setMentionQuery(null)
    setMentionResults([])
  }, [value, connected, contextAttachments, onSend, onChange])

  const canSend = value.trim() && connected

  return (
    <div className="w-full max-w-3xl mx-auto px-4 pb-4 pt-2 relative">
      {/* ── @mention Popover ── */}
      {mentionQuery !== null && (
        <div className="absolute bottom-full left-4 right-4 mb-1 z-50 max-h-64 overflow-y-auto rounded-xl border bg-popover shadow-lg">
          <div className="p-1.5">
            {mentionResults.length > 0 ? (
              mentionResults.map((result, i) => (
              <button
                key={`${result.type}-${result.id}`}
                type="button"
                onClick={() => insertMention(result)}
                className={cn(
                  'flex items-start gap-2.5 w-full rounded-lg px-2.5 py-2 text-left transition-colors',
                  i === mentionIndex ? 'bg-accent text-accent-foreground' : 'hover:bg-accent/50',
                )}
              >
                {result.type === 'mount' ? (
                  <HardDrive className="h-4 w-4 mt-0.5 shrink-0 text-emerald-500" />
                ) : result.type === 'knowledge' ? (
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
                  {result.type === 'mount'
                    ? 'Mount'
                    : result.type === 'knowledge'
                      ? 'KB'
                      : 'Memory'}
                </span>
              </button>
            ))
            ) : (
              <p className="px-2.5 py-3 text-xs text-muted-foreground text-center">
                {mentionQuery === '' ? t('chat.mentionHint') : t('chat.noMentionResults')}
              </p>
            )}
          </div>
        </div>
      )}

      {/* ── Context chips (unified: mounts first, then attachments) ── */}
      {(activeMounts.length > 0 || contextAttachments.length > 0) && (
        <div className="flex flex-wrap gap-1.5 mb-2">
          {activeMounts.map((m) => (
            <span
              key={`mount-${m.id}`}
              className="inline-flex items-center gap-1 rounded-full bg-primary/10 border border-primary/20 px-2.5 py-0.5 text-xs text-primary"
              title={t('chat.mountBound', 'Bound to this session')}
            >
              <HardDrive className="h-3 w-3" />
              <span className="truncate max-w-[140px]">{m.label}</span>
              <button
                type="button"
                onClick={() => onRemoveMount(m.id)}
                className="ml-0.5 -mr-1 rounded-full p-0.5 hover:bg-primary/20 transition-colors"
                aria-label={t('chat.removeMount', 'Remove mount')}
              >
                <X className="h-2.5 w-2.5" />
              </button>
            </span>
          ))}
          {contextAttachments.map((ctx) => (
            <span
              key={`${ctx.type}-${ctx.id}`}
              className="inline-flex items-center gap-1 rounded-full bg-muted/80 px-2.5 py-0.5 text-xs text-foreground"
            >
              {ctx.type === 'knowledge' ? (
                <BookOpen className="h-3 w-3 text-blue-500" />
              ) : (
                <Brain className="h-3 w-3 text-purple-500" />
              )}
              <span className="truncate max-w-[140px]">{ctx.label}</span>
              <button
                type="button"
                onClick={() => removeAttachment(ctx.id)}
                className="ml-0.5 -mr-1 rounded-full p-0.5 hover:bg-muted-foreground/20 transition-colors"
                aria-label={t('chat.removeAttachment', 'Remove attachment')}
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
          'rounded-lg border bg-background shadow-sm transition-all',
          'focus-within:shadow-md focus-within:border-primary/40 focus-within:ring-1 focus-within:ring-ring/30',
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
              ? t('chat.inputPlaceholder', 'Message Oxios… (@ to add context)')
              : t('chat.waitingForConnection', 'Waiting for connection...')
          }
          disabled={disabled || !connected}
          rows={1}
          className={cn(
            'block w-full resize-none bg-transparent px-4 py-3.5 text-sm',
            'placeholder:text-muted-foreground/70',
            'focus:outline-none disabled:cursor-not-allowed',
            'max-h-[280px] overflow-y-auto',
          )}
        />

        {/* ── Bottom bar (flex, not absolute) ── */}
        <div className="flex items-center justify-between gap-2 px-3 pb-2.5 pt-1.5">
          <div className="flex items-center gap-1.5 min-w-0 flex-1">
            <ModelPickerContainer
              activeModelId={activeModelId}
              setActiveModelId={setActiveModelId}
              roles={roles}
              activeRole={activeRole}
              setActiveRole={setActiveRole}
            />
          </div>
          <div className="flex items-center shrink-0 gap-1.5">
            {queuedCount > 0 && (
              <span className="mr-0.5 flex items-center gap-1 rounded-full bg-muted px-2 py-0.5 text-2xs text-muted-foreground">
                <Clock className="h-3 w-3" />
                {t('chat.queued', { count: queuedCount, defaultValue: '{{count}} queued' })}
              </span>
            )}
            {isStreaming && (
              <Button
                onClick={onCancel}
                variant="destructive"
                size="sm"
                className="h-8 rounded-lg px-3 text-xs gap-1.5"
                aria-label={t('chat.stop', 'Stop')}
                title={t('chat.stop', 'Stop')}
              >
                <Square className="h-3 w-3 fill-current" />
                {t('chat.stop', 'Stop')}
              </Button>
            )}
            <Button
              onClick={handleSend}
              disabled={!canSend}
              size="icon"
              className={cn(
                'h-8 w-8 rounded-lg transition-all',
                canSend
                  ? 'bg-primary text-primary-foreground hover:bg-primary/90 shadow-sm'
                  : 'bg-muted text-muted-foreground',
              )}
              aria-label={isStreaming ? t('chat.queue', 'Queue') : t('common.sendMessage', 'Send')}
              title={isStreaming ? t('chat.queue', 'Queue') : t('common.sendMessage', 'Send')}
            >
              <Send className="h-3.5 w-3.5" />
            </Button>
          </div>
        </div>
      </div>

      {/* ── Hint ── */}
      <div className="mt-1.5 flex items-center justify-center gap-3 text-2xs text-muted-foreground/70 hidden sm:flex">
        <Hint kbd="Enter" label={t('chat.send', 'send')} />
        <Hint kbd="Shift+Enter" label={t('chat.newline', 'new line')} />
        <Hint kbd="⌘⇧N" label={t('chat.newConversation', 'new chat')} />
      </div>
    </div>
  )
}

function Hint({ kbd, label }: { kbd: string; label: string }) {
  return (
    <span className="inline-flex items-center gap-1">
      <kbd className="rounded border bg-muted/60 px-1.5 py-0.5 text-2xs font-mono text-muted-foreground">
        {kbd}
      </kbd>
      <span>{label}</span>
    </span>
  )
}
