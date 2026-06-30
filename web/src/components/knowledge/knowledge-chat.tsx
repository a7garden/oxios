import type { LucideIcon } from 'lucide-react'
import {
  BookOpen,
  CheckSquare,
  Clock,
  CornerDownLeft,
  Inbox,
  MessageSquare,
  Newspaper,
  ShoppingCart,
  Square,
  Trash2,
  Tv,
  X,
} from 'lucide-react'
import md5 from 'md5'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { Textarea } from '@/components/ui/textarea'
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

/** Capture-time routing destinations. */
type RouteKey = 'Later' | 'Read' | 'Shop' | 'Watch' | 'Journal'

interface CaptureRoute {
  key: RouteKey
  labelKey: string
  icon: LucideIcon
  /** Checklist file path; absent for Journal (uses journal_add). */
  path?: string
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
  return md5(firstLine).slice(0, 11)
}

// ── Destinations ──────────────────────────────────────────────

/** Per-row "move out of inbox" targets (checklist files). */
const CHECKLIST_TARGETS = [
  { labelKey: 'knowledge.later', icon: Clock, path: 'Later.md' },
  { labelKey: 'knowledge.read', icon: Newspaper, path: 'Read.md' },
  { labelKey: 'knowledge.shop', icon: ShoppingCart, path: 'Shop.md' },
  { labelKey: 'knowledge.watch', icon: Tv, path: 'Watch.md' },
] as const

/** Capture-time routes (slash menu). Journal routes via journal_add. */
const CAPTURE_ROUTES: CaptureRoute[] = [
  { key: 'Later', labelKey: 'knowledge.later', icon: Clock, path: 'Later.md' },
  { key: 'Read', labelKey: 'knowledge.read', icon: Newspaper, path: 'Read.md' },
  { key: 'Shop', labelKey: 'knowledge.shop', icon: ShoppingCart, path: 'Shop.md' },
  { key: 'Watch', labelKey: 'knowledge.watch', icon: Tv, path: 'Watch.md' },
  { key: 'Journal', labelKey: 'knowledge.toJournal', icon: BookOpen },
]

/** Tint classes per route for chips/badges. */
const ROUTE_TINT: Record<RouteKey, string> = {
  Later: 'text-info bg-info-muted',
  Read: 'text-warning bg-warning-muted',
  Shop: 'text-destructive bg-destructive/10',
  Watch: 'text-chart-4 bg-chart-4/10',
  Journal: 'text-success bg-success-muted',
}

// ── Component ─────────────────────────────────────────────────

export function KnowledgeChat() {
  const { t } = useTranslation()
  const { data: rawMessages, isLoading } = useChatMessages()
  const chatAppend = useChatAppend()
  const chatDelete = useChatDelete()
  const journalAdd = useJournalAdd()
  const checklistAdd = useChecklistAdd()

  const [input, setInput] = useState('')
  const [route, setRoute] = useState<RouteKey | null>(null)
  const [routeMenuOpen, setRouteMenuOpen] = useState(false)
  const [hoveredIndex, setHoveredIndex] = useState<number | null>(null)
  const [selectedIndices, setSelectedIndices] = useState<Set<number>>(new Set())
  const [lastClickedIndex, setLastClickedIndex] = useState<number | null>(null)

  const scrollRef = useRef<HTMLDivElement>(null)
  const textareaRef = useRef<HTMLTextAreaElement>(null)
  const barRef = useRef<HTMLDivElement>(null)
  const isDragging = useRef(false)
  const dragStart = useRef<number | null>(null)
  const prevCountRef = useRef(0)

  // ── Grouped messages ──────────────────────────────────────

  const groups = useMemo(() => {
    if (!rawMessages) return []
    return groupMessages(rawMessages)
  }, [rawMessages])

  // Flat parsed list (original order) for selection lookups / bulk ops.
  const flatMessages = useMemo(() => groups.flatMap((g) => g.messages), [groups])

  // Display order: newest-first. Full reverse (group order + within-group)
  // is a strictly-monotone remap of flatMessages, so index-based selection
  // (min/max over msg.index) stays correct without re-mapping.
  const displayGroups = useMemo(
    () => groups.map((g) => ({ ...g, messages: [...g.messages].reverse() })).reverse(),
    [groups],
  )

  // ── Auto-scroll to top when inbox grows ───────────────────
  // Newest sits at the top now, so pin to top on mount and after appends.
  useEffect(() => {
    const count = flatMessages.length
    if (count > prevCountRef.current) {
      scrollRef.current?.scrollTo({ top: 0 })
    }
    prevCountRef.current = count
  }, [flatMessages.length])

  // ── Auto-resize textarea ──────────────────────────────────

  useEffect(() => {
    const el = textareaRef.current
    if (!el) return
    el.style.height = 'auto'
    el.style.height = `${Math.min(el.scrollHeight, 160)}px`
  }, [input])

  // ── Close route menu on outside click ─────────────────────

  useEffect(() => {
    if (!routeMenuOpen) return
    const onDown = (e: MouseEvent) => {
      if (barRef.current && !barRef.current.contains(e.target as Node)) {
        setRouteMenuOpen(false)
      }
    }
    document.addEventListener('mousedown', onDown)
    return () => document.removeEventListener('mousedown', onDown)
  }, [routeMenuOpen])

  // ── Send / shortcuts ──────────────────────────────────────

  const handleSend = useCallback(async () => {
    const text = input.trim()
    if (!text) return

    // Legacy journal shortcut: "some text jj"
    if (text.toLowerCase().endsWith(' jj')) {
      const record = text.slice(0, -3).trim()
      if (record) await journalAdd.mutateAsync(record)
    } else if (route === 'Journal') {
      await journalAdd.mutateAsync(text)
    } else if (route) {
      const target = CAPTURE_ROUTES.find((r) => r.key === route)
      if (target?.path) {
        await checklistAdd.mutateAsync({ path: target.path, item: text })
      } else {
        await chatAppend.mutateAsync(text)
      }
    } else {
      await chatAppend.mutateAsync(text)
    }

    setInput('')
    setRoute(null)
    setRouteMenuOpen(false)
    textareaRef.current?.focus()
  }, [input, route, chatAppend, journalAdd, checklistAdd])

  const pickRoute = useCallback((key: RouteKey | null) => {
    setRoute(key)
    setRouteMenuOpen(false)
    // Strip the leading `/` the user typed to summon the menu.
    setInput((v) => v.replace(/^\//, ''))
    textareaRef.current?.focus()
  }, [])

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      if (e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault()
        handleSend()
      } else if (e.key === 'Escape') {
        setRouteMenuOpen(false)
        if (route) setRoute(null)
      }
    },
    [handleSend, route],
  )

  const handleInputChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const v = e.target.value
    setInput(v)
    if (v === '/' && !route) setRouteMenuOpen(true)
  }

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
  const activeRoute = route ? CAPTURE_ROUTES.find((r) => r.key === route) : null

  // ── Render ────────────────────────────────────────────────

  return (
    <div className="flex flex-col flex-1 h-full">
      {/* Command bar (top) */}
      <div ref={barRef} className="relative shrink-0 border-b px-4 py-3">
        <div
          className={cn(
            'flex items-start gap-1.5 rounded-lg border bg-background px-2.5 py-1.5 transition-shadow',
            'focus-within:border-ring/50 focus-within:ring-2 focus-within:ring-ring/30',
          )}
        >
          {activeRoute && (
            <button
              type="button"
              onClick={() => pickRoute(null)}
              className={cn(
                'mt-0.5 inline-flex shrink-0 items-center gap-1 rounded-md px-1.5 py-1 text-xs font-medium',
                ROUTE_TINT[activeRoute.key],
              )}
            >
              <activeRoute.icon className="h-3.5 w-3.5" />
              {t(activeRoute.labelKey)}
              <X className="h-3 w-3 opacity-60" />
            </button>
          )}

          <Textarea
            ref={textareaRef}
            value={input}
            onChange={handleInputChange}
            onKeyDown={handleKeyDown}
            placeholder={t('knowledge.chatPlaceholder')}
            className="min-h-0 flex-1 resize-none border-0 bg-transparent px-0 py-1 text-sm shadow-none focus-visible:ring-0"
            rows={1}
          />
        </div>

        {/* Hint row — doubles as route-menu toggle */}
        <div className="mt-1.5 flex items-center justify-between px-1">
          <button
            type="button"
            onClick={() => setRouteMenuOpen((o) => !o)}
            className="inline-flex items-center gap-1 text-[11px] text-muted-foreground transition-colors hover:text-foreground"
          >
            <span className="font-mono">/</span>
            {route ? (
              <span className="text-foreground">{t(activeRoute!.labelKey)}</span>
            ) : (
              t('knowledge.routeHint')
            )}
          </button>
          <span className="inline-flex items-center gap-1 text-[11px] text-muted-foreground">
            <CornerDownLeft className="h-3 w-3" />
            {t('knowledge.pressEnterToSend')}
          </span>
        </div>

        {/* Route menu popover */}
        {routeMenuOpen && (
          <div className="absolute left-4 top-full z-20 mt-1 w-56 overflow-hidden rounded-lg border bg-popover p-1 shadow-lg">
            <button
              type="button"
              onClick={() => pickRoute(null)}
              className={cn(
                'flex w-full items-center gap-2.5 rounded-md px-2.5 py-1.5 text-sm transition-colors hover:bg-accent',
                !route && 'bg-accent',
              )}
            >
              <Inbox className="h-4 w-4 text-muted-foreground" />
              <span className="flex-1 text-left">{t('knowledge.inbox')}</span>
              {!route && <CheckSquare className="h-3.5 w-3.5 text-muted-foreground" />}
            </button>
            {CAPTURE_ROUTES.map((r) => {
              const Icon = r.icon
              return (
                <button
                  key={r.key}
                  type="button"
                  onClick={() => pickRoute(r.key)}
                  className={cn(
                    'flex w-full items-center gap-2.5 rounded-md px-2.5 py-1.5 text-sm transition-colors hover:bg-accent',
                    route === r.key && 'bg-accent',
                  )}
                >
                  <Icon className="h-4 w-4 text-muted-foreground" />
                  <span className="flex-1 text-left">{t(r.labelKey)}</span>
                  {route === r.key && <CheckSquare className="h-3.5 w-3.5 text-muted-foreground" />}
                </button>
              )
            })}
          </div>
        )}
      </div>

      {/* Bulk action bar */}
      {hasSelection && (
        <div className="px-4 py-2 border-b bg-muted/50 flex items-center gap-1.5 shrink-0 overflow-x-auto">
          <span className="text-xs text-muted-foreground shrink-0 mr-1">
            {selectedIndices.size} {t('knowledge.selected')}
          </span>
          <Button
            variant="ghost"
            size="sm"
            className="h-7 px-2 text-xs shrink-0"
            onClick={bulkMoveToJournal}
          >
            <BookOpen className="h-3 w-3 mr-1" />
            {t('knowledge.toJournal')}
          </Button>
          {CHECKLIST_TARGETS.map((ct) => (
            <Button
              key={ct.path}
              variant="ghost"
              size="sm"
              className="h-7 px-2 text-xs shrink-0"
              onClick={() => bulkMoveToChecklist(ct.path)}
            >
              <ct.icon className="h-3 w-3 mr-1" />
              {t(ct.labelKey)}
            </Button>
          ))}
          <div className="flex-1 min-w-4" />
          <Button
            variant="ghost"
            size="sm"
            className="h-7 px-2 text-xs text-destructive shrink-0"
            onClick={bulkDelete}
          >
            <Trash2 className="h-3 w-3 mr-1" />
            {t('common.delete')}
          </Button>
        </div>
      )}

      {/* Messages area — newest-first */}
      <div
        ref={scrollRef}
        className="flex-1 overflow-y-auto p-4 select-none"
        onClick={handleBackgroundClick}
      >
        {isLoading ? (
          <div className="text-center text-muted-foreground py-12">{t('knowledge.loading')}</div>
        ) : displayGroups.length === 0 ? (
          <div className="flex flex-col items-center text-muted-foreground py-16">
            <MessageSquare className="h-10 w-10 opacity-20 mb-4" />
            <p className="font-medium text-foreground">{t('knowledge.noFilesYet')}</p>
            <p className="text-sm mt-1">{t('knowledge.dropMindHint')}</p>
          </div>
        ) : (
          <div className="space-y-6">
            {displayGroups.map((group) => (
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
                          'group relative flex items-start gap-2 rounded-lg px-3 py-2 text-sm transition-colors cursor-pointer select-none',
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
                        {/* Completion state — visual only (no in-place toggle endpoint) */}
                        <span className="mt-0.5 shrink-0 text-muted-foreground">
                          {msg.done ? (
                            <CheckSquare className="h-4 w-4 text-success" />
                          ) : (
                            <Square className="h-4 w-4" />
                          )}
                        </span>

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

                        {/* Hover/touch actions */}
                        {((isHovered && !hasSelection) || (isSelected && !isHovered)) &&
                          !isPending && (
                            <div className="flex items-center gap-0.5 shrink-0">
                              {/* To Journal */}
                              <Button
                                variant="ghost"
                                size="icon"
                                className="h-6 w-6"
                                title={t('knowledge.toJournal')}
                                onClick={(e) => {
                                  e.stopPropagation()
                                  moveToJournal(msg)
                                }}
                              >
                                <BookOpen className="h-3.5 w-3.5" />
                              </Button>

                              {/* Checklist targets */}
                              {CHECKLIST_TARGETS.map((ct) => (
                                <Button
                                  key={ct.path}
                                  variant="ghost"
                                  size="icon"
                                  className="h-6 w-6"
                                  title={t(ct.labelKey)}
                                  onClick={(e) => {
                                    e.stopPropagation()
                                    moveToChecklist(ct.path, msg)
                                  }}
                                >
                                  <ct.icon className="h-3.5 w-3.5" />
                                </Button>
                              ))}

                              {/* Delete */}
                              <Button
                                variant="ghost"
                                size="icon"
                                className="h-6 w-6 text-destructive"
                                title={t('common.delete')}
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
      </div>
    </div>
  )
}
