import { Bell } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { useNotificationCenter } from '@/stores/notification-center'
import { useNotificationStore } from '@/stores/notifications'

/**
 * Global notification bell — header trigger for the Notification Center.
 *
 * Opens the slide-over on the notifications tab. The unread badge stays on
 * the bell; the full notification list lives inside the center panel.
 */
export function NotificationBell() {
  const { t } = useTranslation()
  const toggleCenter = useNotificationCenter((s) => s.toggleCenter)
  const open = useNotificationCenter((s) => s.open)
  const activeTab = useNotificationCenter((s) => s.activeTab)
  const unreadCount = useNotificationStore((s) => s.unreadCount)

  const isActive = open && activeTab === 'notifications'

  return (
    <button
      type="button"
      onClick={() => toggleCenter('notifications')}
      className="relative rounded-md p-2 hover:bg-accent/50 transition-all focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
      aria-label={`${t('notifications.openNotifications')}${unreadCount > 0 ? t('notifications.unreadCount', { count: unreadCount }) : ''}`}
      aria-pressed={isActive}
    >
      <Bell className="h-4 w-4" />
      {unreadCount > 0 && (
        <span className="absolute -top-0.5 -right-0.5 flex h-4 min-w-4 items-center justify-center rounded-full bg-destructive px-1 text-2xs font-bold text-destructive-foreground animate-scale-in">
          {unreadCount > 99 ? '99+' : unreadCount}
        </span>
      )}
    </button>
  )
}
