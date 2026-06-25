import { useNavigate } from '@tanstack/react-router'
import type { TFunction } from 'i18next'
import { Bell, Check, Plus, X } from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { EventDetail } from '@/components/calendar/event-detail'
import { EventEditor } from '@/components/calendar/event-editor'
import { MiniCalendar } from '@/components/calendar/mini-calendar'
import { Button } from '@/components/ui/button'
import { ScrollArea } from '@/components/ui/scroll-area'
import {
  useCalendarCreate,
  useCalendarDelete,
  useCalendarEvents,
  useCalendarUpdate,
} from '@/hooks/use-calendar'
import { cn } from '@/lib/utils'
import { type CenterTab, useNotificationCenter } from '@/stores/notification-center'
import {
  type Notification,
  type NotificationSeverity,
  useNotificationStore,
} from '@/stores/notifications'
import type { CalendarEvent, CreateEventRequest, UpdateEventRequest } from '@/types/calendar'

// ─── Notifications-tab helpers (ported from the old inline bell dropdown) ──

const SEVERITY_DOT: Record<NotificationSeverity, string> = {
  info: 'bg-info',
  warning: 'bg-warning',
  error: 'bg-error',
  success: 'bg-success',
}

/** i18n-aware relative time formatter. */
function timeAgo(iso: string, t: TFunction): string {
  const diff = Date.now() - new Date(iso).getTime()
  if (diff < 60_000) return t('common.justNow', 'just now')
  if (diff < 3_600_000) return t('common.minutesAgo', { count: Math.floor(diff / 60_000) })
  if (diff < 86_400_000) return t('common.hoursAgo', { count: Math.floor(diff / 3_600_000) })
  return t('common.daysAgo', { count: Math.floor(diff / 86_400_000) })
}

// ─── Date helpers ─────────────────────────────────────────────────────────

function isSameDay(a: Date, b: Date): boolean {
  return (
    a.getFullYear() === b.getFullYear() &&
    a.getMonth() === b.getMonth() &&
    a.getDate() === b.getDate()
  )
}

/** First cell (Sunday) of the 6×7 grid for the month of `anchor`. */
function gridStart(anchor: Date): Date {
  const first = new Date(anchor.getFullYear(), anchor.getMonth(), 1)
  return new Date(first.getFullYear(), first.getMonth(), first.getDate() - first.getDay())
}

/** `YYYY-MM-DD` local key (own local time, no TZ shift). */
function dateKey(d: Date): string {
  const p = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`
}

// ─── Shell ─────────────────────────────────────────────────────────────────

/**
 * Notification Center — macOS-style right slide-over unifying the schedule
 * (calendar) and the notification feed behind two tabs.
 *
 * Always mounted (so the slide transition can play on close); the tab content
 * is cheap enough to keep warm, giving instant data when opened.
 */
export function NotificationCenter() {
  const { t } = useTranslation()
  const open = useNotificationCenter((s) => s.open)
  const activeTab = useNotificationCenter((s) => s.activeTab)
  const setTab = useNotificationCenter((s) => s.setTab)
  const closeCenter = useNotificationCenter((s) => s.closeCenter)
  const unreadCount = useNotificationStore((s) => s.unreadCount)

  // Escape closes — only while open.
  useEffect(() => {
    if (!open) return
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') closeCenter()
    }
    document.addEventListener('keydown', onKey)
    return () => document.removeEventListener('keydown', onKey)
  }, [open, closeCenter])

  const tabs: { id: CenterTab; label: string; badge?: number }[] = [
    { id: 'schedule', label: t('notificationCenter.schedule') },
    { id: 'notifications', label: t('notificationCenter.notifications'), badge: unreadCount },
  ]

  return (
    <>
      {/* Backdrop */}
      <div
        role="presentation"
        aria-hidden={!open}
        onClick={closeCenter}
        className={cn(
          'fixed inset-0 z-40 bg-black/40 backdrop-blur-[2px]',
          'transition-opacity duration-300 ease-[var(--animate-in-easing)]',
          open ? 'opacity-100' : 'pointer-events-none opacity-0',
        )}
      />

      {/* Slide-over panel */}
      <aside
        role="dialog"
        aria-modal="false"
        aria-label={t('notificationCenter.title')}
        className={cn(
          'fixed inset-y-0 right-0 z-50 flex w-[380px] max-w-[calc(100vw-1.5rem)] flex-col',
          'border-l bg-background shadow-2xl',
          'transition-transform duration-300 ease-[var(--animate-in-easing)] will-change-transform',
          'pt-[env(safe-area-inset-top)] pb-[env(safe-area-inset-bottom)]',
          open ? 'translate-x-0' : 'pointer-events-none translate-x-full',
        )}
      >
        {/* Header: tabs */}
        <div className="flex items-center gap-1 border-b px-3 py-2">
          <div className="flex flex-1 items-center gap-1">
            {tabs.map((tab) => (
              <button
                key={tab.id}
                type="button"
                onClick={() => setTab(tab.id)}
                className={cn(
                  'relative rounded-md px-3 py-1.5 text-sm transition-colors',
                  'focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring',
                  activeTab === tab.id
                    ? 'bg-accent text-accent-foreground font-medium'
                    : 'text-muted-foreground hover:bg-accent/50',
                )}
              >
                {tab.label}
                {tab.badge ? (
                  <span className="ml-1.5 inline-flex h-4 min-w-4 items-center justify-center rounded-full bg-destructive px-1 text-2xs font-bold text-destructive-foreground">
                    {tab.badge > 99 ? '99+' : tab.badge}
                  </span>
                ) : null}
              </button>
            ))}
          </div>
          <Button variant="ghost" size="icon" className="h-8 w-8" onClick={closeCenter}>
            <X className="h-4 w-4" />
          </Button>
        </div>

        {/* Body */}
        <ScrollArea className="flex-1 min-h-0">
          {activeTab === 'schedule' ? <ScheduleTab /> : <NotificationsTab />}
        </ScrollArea>
      </aside>
    </>
  )
}

// ─── Schedule tab ──────────────────────────────────────────────────────────

function ScheduleTab() {
  const { t, i18n } = useTranslation()
  const now = useMemo(() => new Date(), [])
  const [viewAnchor, setViewAnchor] = useState(() => new Date())
  const [selectedDate, setSelectedDate] = useState<Date>(now)
  const [editorOpen, setEditorOpen] = useState(false)
  const [editingEvent, setEditingEvent] = useState<CalendarEvent | undefined>()
  const [defaultStart, setDefaultStart] = useState<Date | undefined>()
  const [detailEvent, setDetailEvent] = useState<CalendarEvent | null>(null)

  // Query the full 6×7 grid span so every visible cell has event data.
  const { from, to } = useMemo(() => {
    const start = gridStart(viewAnchor)
    const end = new Date(start)
    end.setDate(start.getDate() + 42)
    return { from: start.toISOString(), to: end.toISOString() }
  }, [viewAnchor])

  const { data, isLoading } = useCalendarEvents(from, to)
  const events = useMemo(() => (Array.isArray(data?.events) ? data.events : []), [data])

  const createMutation = useCalendarCreate()
  const updateMutation = useCalendarUpdate()
  const deleteMutation = useCalendarDelete()

  // Agenda: events on the selected day, sorted by start time.
  const dayEvents = useMemo(() => {
    const key = dateKey(selectedDate)
    return events
      .filter((e) => dateKey(new Date(e.start)) === key)
      .sort((a, b) => new Date(a.start).getTime() - new Date(b.start).getTime())
  }, [events, selectedDate])

  // Next upcoming event across the loaded window.
  const nextEvent = useMemo(() => {
    const upcoming = events
      .filter((e) => new Date(e.start).getTime() >= now.getTime())
      .sort((a, b) => new Date(a.start).getTime() - new Date(b.start).getTime())
    return upcoming[0]
  }, [events, now])

  const openCreate = (date?: Date) => {
    setEditingEvent(undefined)
    setDefaultStart(date ?? selectedDate)
    setEditorOpen(true)
  }

  const handleSubmit = (data: CreateEventRequest | UpdateEventRequest) => {
    if (editingEvent) {
      updateMutation.mutate(
        { uid: editingEvent.uid, ...(data as UpdateEventRequest) },
        { onSuccess: () => setEditorOpen(false) },
      )
    } else {
      createMutation.mutate(data as CreateEventRequest, { onSuccess: () => setEditorOpen(false) })
    }
  }

  const isToday = isSameDay(selectedDate, now)
  const selectedLabel = selectedDate.toLocaleDateString(i18n.language, {
    month: 'long',
    day: 'numeric',
    weekday: 'long',
  })

  return (
    <div className="space-y-3 p-3">
      <MiniCalendar
        events={events}
        viewAnchor={viewAnchor}
        onViewAnchorChange={setViewAnchor}
        selectedDate={selectedDate}
        onSelectDate={setSelectedDate}
      />

      {/* Next event banner */}
      {nextEvent && (
        <div className="rounded-lg border bg-accent/30 px-3 py-2">
          <p className="text-2xs font-medium uppercase tracking-wide text-muted-foreground">
            {t('notificationCenter.nextEvent')}
          </p>
          <p className="mt-0.5 truncate text-sm font-medium">{nextEvent.title}</p>
          <p className="text-xs text-muted-foreground">
            {new Date(nextEvent.start).toLocaleString(i18n.language, {
              month: 'short',
              day: 'numeric',
              hour: '2-digit',
              minute: '2-digit',
            })}
          </p>
        </div>
      )}

      {/* Agenda for selected day */}
      <div>
        <div className="mb-1.5 flex items-center justify-between">
          <span className="text-xs font-medium text-muted-foreground">
            {isToday ? t('calendar.today') : selectedLabel}
          </span>
          <Button
            variant="ghost"
            size="sm"
            className="h-6 px-2 text-xs"
            onClick={() => openCreate()}
          >
            <Plus className="mr-1 h-3 w-3" /> {t('calendar.newEvent')}
          </Button>
        </div>

        {isLoading ? (
          <p className="py-4 text-center text-sm text-muted-foreground">{t('calendar.loading')}</p>
        ) : dayEvents.length === 0 ? (
          <p className="py-4 text-center text-sm text-muted-foreground">
            {t('notificationCenter.noUpcoming')}
          </p>
        ) : (
          <div className="space-y-1">
            {dayEvents.map((ev) => (
              <button
                key={ev.uid}
                type="button"
                onClick={() => setDetailEvent(ev)}
                className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left transition-colors hover:bg-accent/50 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
              >
                <span
                  className={cn(
                    'h-2 w-2 shrink-0 rounded-full',
                    ev.source === 'agent'
                      ? 'bg-info'
                      : ev.source === 'cron'
                        ? 'bg-warning'
                        : 'bg-primary',
                  )}
                />
                <span className="shrink-0 text-xs tabular-nums text-muted-foreground">
                  {ev.all_day
                    ? t('calendar.allDay')
                    : new Date(ev.start).toLocaleTimeString(i18n.language, {
                        hour: '2-digit',
                        minute: '2-digit',
                      })}
                </span>
                <span className="min-w-0 flex-1 truncate text-sm">{ev.title}</span>
              </button>
            ))}
          </div>
        )}
      </div>

      <EventEditor
        open={editorOpen}
        onClose={() => setEditorOpen(false)}
        event={editingEvent}
        defaultStart={defaultStart}
        onSubmit={handleSubmit}
        isLoading={createMutation.isPending || updateMutation.isPending}
      />

      {detailEvent && (
        <EventDetail
          event={detailEvent}
          onEdit={() => {
            setEditingEvent(detailEvent)
            setDefaultStart(new Date(detailEvent.start))
            setDetailEvent(null)
            setEditorOpen(true)
          }}
          onDelete={() => {
            deleteMutation.mutate(detailEvent.uid, { onSuccess: () => setDetailEvent(null) })
          }}
          onClose={() => setDetailEvent(null)}
        />
      )}
    </div>
  )
}

// ─── Notifications tab ────────────────────────────────────────────────────

function NotificationsTab() {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const closeCenter = useNotificationCenter((s) => s.closeCenter)

  const notifications = useNotificationStore((s) => s.notifications)
  const unreadCount = useNotificationStore((s) => s.unreadCount)
  const markRead = useNotificationStore((s) => s.markRead)
  const markAllRead = useNotificationStore((s) => s.markAllRead)
  const dismiss = useNotificationStore((s) => s.dismiss)

  const handleClick = (n: Notification) => {
    markRead(n.id)
    if (n.link) {
      closeCenter()
      navigate({ to: n.link })
    }
  }

  return (
    <div className="flex flex-col">
      <div className="flex items-center justify-between border-b px-3 py-2">
        <span className="text-xs text-muted-foreground">
          {unreadCount > 0 ? t('notifications.unreadCount', { count: unreadCount }) : null}
        </span>
        {unreadCount > 0 && (
          <Button variant="ghost" size="sm" className="h-6 text-xs" onClick={markAllRead}>
            <Check className="mr-1 h-3 w-3" /> {t('notifications.markAllRead')}
          </Button>
        )}
      </div>

      {notifications.length === 0 ? (
        <div className="flex flex-col items-center justify-center gap-2 py-12 text-muted-foreground">
          <Bell className="h-8 w-8 opacity-30" />
          <p className="text-sm">{t('notifications.noNotifications')}</p>
        </div>
      ) : (
        <div className="divide-y">
          {notifications.map((n) => (
            // biome-ignore lint/a11y/useSemanticElements: nested dismiss button; div is correct
            <div
              key={n.id}
              className={cn(
                'group flex gap-2 px-3 py-2.5 transition-all cursor-pointer hover:bg-accent/50',
                !n.read && 'bg-accent/20',
              )}
              onClick={() => handleClick(n)}
              role="button"
              tabIndex={0}
              onKeyDown={(e) => {
                if (e.key === 'Enter') handleClick(n)
              }}
            >
              <div
                className={cn('mt-0.5 h-2 w-2 shrink-0 rounded-full', SEVERITY_DOT[n.severity])}
              />
              <div className="min-w-0 flex-1">
                <p className="truncate text-sm font-medium leading-tight">{n.title}</p>
                {n.message && (
                  <p className="mt-0.5 line-clamp-2 text-xs text-muted-foreground">{n.message}</p>
                )}
                <p className="mt-1 text-2xs text-muted-foreground/60">{timeAgo(n.timestamp, t)}</p>
              </div>
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation()
                  dismiss(n.id)
                }}
                className="shrink-0 rounded p-0.5 opacity-0 transition-opacity hover:bg-muted group-hover:opacity-100"
                aria-label={t('common.dismiss')}
              >
                <X className="h-3 w-3" />
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
