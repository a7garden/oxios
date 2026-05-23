import {
  BookOpen,
  CheckSquare,
  Clock,
  Newspaper,
  Send,
  ShoppingCart,
  Square,
  Trash2,
  Tv,
} from 'lucide-react'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { Button } from '@/components/ui/button'
import {
  useChatAppend,
  useChatDelete,
  useChatMessages,
  useChecklistAdd,
  useJournalAdd,
} from '@/hooks/use-knowledge'
import { cn } from '@/lib/utils'

// ── Types ─────────────────────────────────────────────────────

interface ParsedMessage {
  /** Original index in the raw array */
  index: number
  /** Whether `[x]` (done) or `[ ]` (pending) */
  done: boolean
  /** The `HH:MM` timestamp, if present */
  timestamp: string
  /** The message body text */
  text: string
  /** Date header this message belongs to */
  date: string
  /** The raw string from the backend */
  raw: string
}

interface DateGroup {
  date: string
  messages: ParsedMessage[]
}

// ── Parsing ───────────────────────────────────────────────────

const DONE_RE = /^- \[([xX ])\] (?:`(\d{2}:\d{2})` )?(.+)$/
const DATE_HEADER_RE = /^#### (.+)$/

function parseMessage(raw: string, index: number): ParsedMessage | null {
  const m = raw.match(DONE_RE)
  if (!m) return null
  return {
    index,
    done: m[1] === 'x' || m[1] === 'X',
    timestamp: m[2] ?? '',
    text: m[3] ?? '',
    date: '',
    raw,
  }
}

function isDateHeader(raw: string): string | null {
  const m = raw.match(DATE_HEADER_RE)
  return m?.[1] ?? null
}

/**
 * Group raw backend strings into date buckets with parsed messages.
 * Non-parseable lines (not date headers or checklist items) are skipped.
 */
function groupMessages(raws: string[]): DateGroup[] {
  const groups: DateGroup[] = []
  let currentDate = 'Today'

  for (let i = 0; i < raws.length; i++) {
    const raw = raws[i]
    const headerText = isDateHeader(raw!)
    if (headerText) {
      currentDate = headerText
      continue
    }
    const parsed = parseMessage(raw!, i)
    if (!parsed) continue
    parsed.date = currentDate

    let group = groups[groups.length - 1]
    if (!group || group.date !== currentDate) {
      group = { date: currentDate, messages: [] }
      groups.push(group)
    }
    group.messages.push(parsed)
  }

  return groups
}

// ── Simple hash for msg_hash ─────────────────────────────────
// Computes the same MD5(first_line)[..11] hash that the backend uses.
// Backend: oxios-markdown/src/fs.rs → hash_filename() → MD5 → first 11 hex chars.

export async function msgHash(raw: string): Promise<string> {
  const stripped = raw.replace(/^- \[[ xX]\] /, '')
  const firstLine = stripped.split('\n')[0] ?? ''
  try {
    // Web Crypto API — MD5 via md5-js fallback
    const { default: md5 } = await import('md5')
    return md5(firstLine).slice(0, 11)
  } catch {
    // Fallback: simple non-crypto hash (deterministic across runs)
    let h = 5381
    for (let i = 0; i < firstLine.length; i++) {
      h = Math.imul(33, h) ^ firstLine.charCodeAt(i)
    }
    return Math.abs(h >>> 0)
      .toString(16)
      .padStart(11, '0')
      .slice(0, 11)
  }
}

// ── Checklist targets ─────────────────────────────────────────

const CHECKLIST_TARGETS = [
  { label: 'Later', icon: Clock, path: 'Later.md' },
  { label: 'Read', icon: Newspaper, path: 'Read.md' },
  { label: 'Shop', icon: ShoppingCart, path: 'Shop.md' },
  { label: 'Watch', icon: Tv, path: 'Watch.md' },
] as const

// ── Component ─────────────────────────────────────────────────

export function KnowledgeChat() {
  const { data: rawMessages, isLoading } = useChatMessages()
  const chatAppend = useChatAppend()
  const chatDelete = useChatDelete()
  const journalAdd = useJournalAdd()
  const checklistAdd = useChecklistAdd()

  const [input, setInput] = useState('')
  const [hoveredIndex, setHoveredIndex] = useState<number | null>(null)
  const [selectedIndices, setSelectedIndices] = useState<Set<number>>(new Set())
  const [lastClickedIndex, setLastClickedIndex] = useState<number | null>(null)

  const messagesEndRef = useRef<HTMLDivElement>(null)
  const textareaRef = useRef<HTMLTextAreaElement>(null)
  const isDragging = useRef(false)
  const dragStart = useRef<number | null>(null)

  // ── Grouped messages ──────────────────────────────────────

  const groups = useMemo(() => {
    if (!rawMessages) return []
    return groupMessages(rawMessages)
  }, [rawMessages])

  // Flat parsed list for selection lookups
  const flatMessages = useMemo(() => groups.flatMap((g) => g.messages), [groups])

  // ── Auto-scroll ───────────────────────────────────────────

  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [])

  // ── Auto-resize textarea ──────────────────────────────────

  useEffect(() => {
    const el = textareaRef.current
    if (!el) return
    el.style.height = 'auto'
    el.style.height = `${Math.min(el.scrollHeight, 160)}px`
  }, [])

  // ── Send / shortcuts ──────────────────────────────────────

  const handleSend = useCallback(async () => {
    const text = input.trim()
    if (!text) return

    // Journal shortcut: "some text jj"
    if (text.toLowerCase().endsWith(' jj')) {
      const record = text.slice(0, -3).trim()
      if (record) {
        await journalAdd.mutateAsync(record)
      }
      setInput('')
      return
    }

    await chatAppend.mutateAsync(text)
    setInput('')
    textareaRef.current?.focus()
  }, [input, chatAppend, journalAdd])

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault()
        handleSend()
      }
    },
    [handleSend],
  )

  // ── Selection ─────────────────────────────────────────────

  const handleMessageClick = useCallback(
    (msg: ParsedMessage, e: React.MouseEvent) => {
      e.stopPropagation()

      setSelectedIndices((prev) => {
        const next = new Set(prev)

        if (e.shiftKey && lastClickedIndex !== null) {
          // Range select
          const from = Math.min(lastClickedIndex, msg.index)
          const to = Math.max(lastClickedIndex, msg.index)
          for (let i = from; i <= to; i++) next.add(i)
        } else if (e.metaKey || e.ctrlKey) {
          // Toggle individual
          if (next.has(msg.index)) {
            next.delete(msg.index)
          } else {
            next.add(msg.index)
          }
        } else {
          // Single select / deselect
          if (next.size === 1 && next.has(msg.index)) {
            next.clear()
          } else {
            next.clear()
            next.add(msg.index)
          }
        }
        return next
      })

      setLastClickedIndex(msg.index)
    },
    [lastClickedIndex],
  )

  // Clear selection on background click
  const handleBackgroundClick = useCallback(() => {
    setSelectedIndices(new Set())
    setLastClickedIndex(null)
  }, [])

  // ── Drag selection ────────────────────────────────────────

  const handleDragStart = useCallback((msg: ParsedMessage) => {
    isDragging.current = true
    dragStart.current = msg.index
    setSelectedIndices(new Set([msg.index]))
  }, [])

  const handleDragEnter = useCallback((msg: ParsedMessage) => {
    if (!isDragging.current || dragStart.current === null) return
    const from = Math.min(dragStart.current, msg.index)
    const to = Math.max(dragStart.current, msg.index)
    const next = new Set<number>()
    for (let i = from; i <= to; i++) next.add(i)
    setSelectedIndices(next)
  }, [])

  const handleDragEnd = useCallback(() => {
    isDragging.current = false
    dragStart.current = null
  }, [])

  // ── Actions ───────────────────────────────────────────────

  const moveToJournal = useCallback(
    async (msg: ParsedMessage) => {
      await journalAdd.mutateAsync(msg.text)
      await chatDelete.mutateAsync(await msgHash(msg.raw))
    },
    [journalAdd, chatDelete],
  )

  const moveToChecklist = useCallback(
    async (path: string, msg: ParsedMessage) => {
      await checklistAdd.mutateAsync({ path, item: msg.text })
      await chatDelete.mutateAsync(await msgHash(msg.raw))
    },
    [checklistAdd, chatDelete],
  )

  const deleteMessage = useCallback(
    async (msg: ParsedMessage) => {
      await chatDelete.mutateAsync(await msgHash(msg.raw))
    },
    [chatDelete],
  )

  // Bulk actions on selected messages
  const bulkMoveToChecklist = useCallback(
    async (path: string) => {
      const targets = flatMessages.filter((m) => selectedIndices.has(m.index))
      for (const msg of targets) {
        await checklistAdd.mutateAsync({ path, item: msg.text })
        await chatDelete.mutateAsync(await msgHash(msg.raw))
      }
      setSelectedIndices(new Set())
    },
    [flatMessages, selectedIndices, checklistAdd, chatDelete],
  )

  const bulkMoveToJournal = useCallback(async () => {
    const targets = flatMessages.filter((m) => selectedIndices.has(m.index))
    for (const msg of targets) {
      await journalAdd.mutateAsync(msg.text)
      await chatDelete.mutateAsync(await msgHash(msg.raw))
    }
    setSelectedIndices(new Set())
  }, [flatMessages, selectedIndices, journalAdd, chatDelete])

  const bulkDelete = useCallback(async () => {
    const targets = flatMessages.filter((m) => selectedIndices.has(m.index))
    for (const msg of targets) {
      await chatDelete.mutateAsync(await msgHash(msg.raw))
    }
    setSelectedIndices(new Set())
  }, [flatMessages, selectedIndices, chatDelete])

  const hasSelection = selectedIndices.size > 0

  // ── Render ────────────────────────────────────────────────

  return (
    <div className="flex flex-col flex-1 h-full">
      {/* Header */}
      <div className="px-4 py-2 border-b shrink-0 bg-muted/30">
        <p className="text-xs text-muted-foreground">Free your head</p>
      </div>

      {/* Bulk action bar */}
      {hasSelection && (
        <div className="px-4 py-2 border-b bg-muted/50 flex items-center gap-2 shrink-0">
          <span className="text-xs text-muted-foreground mr-1">
            {selectedIndices.size} selected
          </span>
          <Button
            variant="ghost"
            size="sm"
            className="h-7 px-2 text-xs"
            onClick={bulkMoveToJournal}
          >
            <BookOpen className="h-3 w-3 mr-1" />
            Journal
          </Button>
          {CHECKLIST_TARGETS.map((t) => (
            <Button
              key={t.path}
              variant="ghost"
              size="sm"
              className="h-7 px-2 text-xs"
              onClick={() => bulkMoveToChecklist(t.path)}
            >
              <t.icon className="h-3 w-3 mr-1" />
              {t.label}
            </Button>
          ))}
          <div className="flex-1" />
          <Button
            variant="ghost"
            size="sm"
            className="h-7 px-2 text-xs text-destructive"
            onClick={bulkDelete}
          >
            <Trash2 className="h-3 w-3 mr-1" />
            Delete
          </Button>
        </div>
      )}

      {/* Messages area */}
      <div className="flex-1 overflow-y-auto p-4 select-none" onClick={handleBackgroundClick}>
        {isLoading ? (
          <div className="text-center text-muted-foreground py-12">Loading…</div>
        ) : groups.length === 0 ? (
          <div className="text-center text-muted-foreground py-12">
            <p className="text-2xl mb-2">🌱</p>
            <p className="font-medium">Free your head</p>
            <p className="text-sm">Drop whatever's on your mind here</p>
          </div>
        ) : (
          <div className="space-y-6">
            {groups.map((group) => (
              <div key={group.date}>
                {/* Date header */}
                <div className="sticky top-0 z-10 bg-background/80 backdrop-blur-sm py-1 mb-2">
                  <p className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
                    {group.date}
                  </p>
                </div>

                {/* Messages */}
                <div className="space-y-1">
                  {group.messages.map((msg) => {
                    const isHovered = hoveredIndex === msg.index
                    const isSelected = selectedIndices.has(msg.index)
                    const isPending =
                      chatDelete.isPending || checklistAdd.isPending || journalAdd.isPending

                    return (
                      <div
                        key={msg.index}
                        className={cn(
                          'group relative flex items-start gap-2 rounded-lg px-3 py-2 text-sm transition-colors cursor-pointer',
                          isSelected
                            ? 'bg-primary/10 ring-1 ring-primary/30'
                            : 'hover:bg-accent/40',
                        )}
                        onClick={(e) => handleMessageClick(msg, e)}
                        onMouseEnter={() => setHoveredIndex(msg.index)}
                        onMouseLeave={() => setHoveredIndex(null)}
                        draggable
                        onDragStart={() => handleDragStart(msg)}
                        onDragEnter={() => handleDragEnter(msg)}
                        onDragEnd={handleDragEnd}
                      >
                        {/* Checkbox — click to toggle completion */}
                        <button
                          type="button"
                          className="mt-0.5 shrink-0 text-muted-foreground hover:text-foreground transition-colors"
                          title={msg.done ? 'Mark incomplete' : 'Mark complete'}
                          disabled={chatDelete.isPending || chatAppend.isPending}
                          onClick={async (e) => {
                            e.stopPropagation()
                            const hash = await msgHash(msg.raw)
                            const oldPrefix = msg.done ? '- [x]' : '- [ ]'
                            const newPrefix = msg.done ? '- [ ]' : '- [x]'
                            const rest = msg.raw.replace(oldPrefix, '').trim()
                            await chatDelete.mutateAsync(hash)
                            await chatAppend.mutateAsync(`${newPrefix} ${rest}`)
                          }}
                        >
                          {msg.done ? (
                            <CheckSquare className="h-4 w-4 text-emerald-500" />
                          ) : (
                            <Square className="h-4 w-4" />
                          )}
                        </button>

                        {/* Timestamp */}
                        {msg.timestamp && (
                          <span className="shrink-0 text-xs text-muted-foreground font-mono tabular-nums mt-0.5">
                            {msg.timestamp}
                          </span>
                        )}

                        {/* Text */}
                        <span
                          className={cn(
                            'flex-1 whitespace-pre-wrap break-words',
                            msg.done && 'line-through text-muted-foreground',
                          )}
                        >
                          {msg.text}
                        </span>

                        {/* Hover actions */}
                        {isHovered && !hasSelection && !isPending && (
                          <div className="flex items-center gap-0.5 shrink-0">
                            {/* To Journal */}
                            <Button
                              variant="ghost"
                              size="icon"
                              className="h-6 w-6"
                              title="To Journal"
                              onClick={(e) => {
                                e.stopPropagation()
                                moveToJournal(msg)
                              }}
                            >
                              <BookOpen className="h-3.5 w-3.5" />
                            </Button>

                            {/* Checklist targets */}
                            {CHECKLIST_TARGETS.map((t) => (
                              <Button
                                key={t.path}
                                variant="ghost"
                                size="icon"
                                className="h-6 w-6"
                                title={`To ${t.label}`}
                                onClick={(e) => {
                                  e.stopPropagation()
                                  moveToChecklist(t.path, msg)
                                }}
                              >
                                <t.icon className="h-3.5 w-3.5" />
                              </Button>
                            ))}

                            {/* Delete */}
                            <Button
                              variant="ghost"
                              size="icon"
                              className="h-6 w-6 text-destructive"
                              title="Delete"
                              onClick={(e) => {
                                e.stopPropagation()
                                deleteMessage(msg)
                              }}
                            >
                              <Trash2 className="h-3.5 w-3.5" />
                            </Button>
                          </div>
                        )}
                      </div>
                    )
                  })}
                </div>
              </div>
            ))}
          </div>
        )}
        <div ref={messagesEndRef} />
      </div>

      {/* Input area */}
      <div className="border-t p-3 shrink-0">
        <div className="flex gap-2 items-end">
          <textarea
            ref={textareaRef}
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Type a message… (jj for journal)"
            className={cn(
              'flex-1 resize-none rounded-md border bg-background px-3 py-2 text-sm',
              'focus:outline-none focus:ring-1 focus:ring-primary',
              'max-h-40 overflow-y-auto',
            )}
            rows={1}
          />
          <Button
            onClick={handleSend}
            disabled={!input.trim() || chatAppend.isPending || journalAdd.isPending}
            size="icon"
          >
            <Send className="h-4 w-4" />
          </Button>
        </div>
      </div>
    </div>
  )
}
