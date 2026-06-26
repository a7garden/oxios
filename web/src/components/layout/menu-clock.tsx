import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { cn } from '@/lib/utils'
import { useNotificationCenter } from '@/stores/notification-center'
import { useNotificationStore } from '@/stores/notifications'
import { useHour12 } from '@/stores/ui-prefs'

/**
 * Menu-bar clock — the single trigger for the Notification Center.
 *
 * Mimics the macOS menu-bar: shows weekday + month + day + current time,
 * with an unread-notification badge. Clicking opens the unified Notification
 * Center panel (calendar widget + notification inbox).
 *
 * Replaces the old separate CalendarTrigger + NotificationBell pair.
 */
export function MenuClock() {
  const { t, i18n } = useTranslation()
  const toggleCenter = useNotificationCenter((s) => s.toggleCenter)
  const open = useNotificationCenter((s) => s.open)
  const unreadCount = useNotificationStore((s) => s.unreadCount)
  const hour12 = useHour12()

  // Live clock — ticks every second so the time stays accurate.
  const [now, setNow] = useState(() => new Date())
  useEffect(() => {
    const id = setInterval(() => setNow(new Date()), 1000)
    return () => clearInterval(id)
  }, [])

  const dateLabel = now.toLocaleDateString(i18n.language, {
    weekday: 'short',
    month: 'short',
    day: 'numeric',
  })
  const timeLabel = now.toLocaleTimeString(i18n.language, {
    hour: '2-digit',
    minute: '2-digit',
    hour12,
  })

  return (
    <button
      type="button"
      onClick={toggleCenter}
      className={cn(
        'flex items-center gap-2 rounded-md px-2.5 py-1.5 text-sm transition-all',
        'focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring',
        open ? 'bg-accent text-accent-foreground' : 'hover:bg-accent/50',
      )}
      aria-label={`${dateLabel} ${timeLabel} — ${t('notificationCenter.title')}${unreadCount > 0 ? t('common.unreadCount', { count: unreadCount }) : ''}`}
      aria-pressed={open}
    >
      <span className="text-muted-foreground">{dateLabel}</span>
      <span className="tabular-nums font-medium">{timeLabel}</span>
      {unreadCount > 0 && (
        <span className="flex h-4 min-w-4 items-center justify-center rounded-full bg-destructive px-1 text-2xs font-bold text-destructive-foreground animate-scale-in">
          {unreadCount > 99 ? '99+' : unreadCount}
        </span>
      )}
    </button>
  )
}
