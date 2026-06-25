import { CalendarDays } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { useNotificationCenter } from '@/stores/notification-center'

/**
 * Header calendar icon — opens the Notification Center on the schedule tab.
 *
 * Sits next to the {@link NotificationBell}. Both controls drive the same
 * slide-over panel; this one defaults to the schedule (calendar) tab.
 */
export function CalendarTrigger() {
  const { t } = useTranslation()
  const toggleCenter = useNotificationCenter((s) => s.toggleCenter)
  const open = useNotificationCenter((s) => s.open)
  const activeTab = useNotificationCenter((s) => s.activeTab)

  const isActive = open && activeTab === 'schedule'

  return (
    <button
      type="button"
      onClick={() => toggleCenter('schedule')}
      className="relative rounded-md p-2 hover:bg-accent/50 transition-all focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
      aria-label={t('notificationCenter.openCalendar')}
      aria-pressed={isActive}
    >
      <CalendarDays className="h-4 w-4" />
    </button>
  )
}
