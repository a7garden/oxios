/**
 * Desktop notification utilities (RFC-028 SP-1d).
 *
 * Uses the browser Notification API to show OS-level notifications when
 * the Oxios tab is in the background. Foreground events rely on in-app
 * toasts instead.
 */

/**
 * Request notification permission from the user.
 * Returns `true` if permission is granted (or was already granted).
 */
export async function requestNotificationPermission(): Promise<boolean> {
  if (!('Notification' in window)) return false
  if (Notification.permission === 'granted') return true
  if (Notification.permission === 'denied') return false
  const result = await Notification.requestPermission()
  return result === 'granted'
}

/**
 * Show a desktop notification. Only fires when:
 * - The tab is hidden (background) — foreground gets in-app toast.
 * - Notification permission is granted.
 *
 * @param title Notification title
 * @param body  Notification body text
 * @param link  Optional hash route to navigate to on click (e.g. "/agents")
 */
export function showDesktopNotification(title: string, body: string, link?: string) {
  // Foreground tab: toast is sufficient, skip desktop notification.
  if (!document.hidden) return
  if (!('Notification' in window)) return
  if (Notification.permission !== 'granted') return

  const n = new Notification(title, {
    body,
    tag: 'oxios-agent',
    // Reuse favicon as the notification icon.
    icon: '/favicon.svg',
  })

  n.onclick = () => {
    window.focus()
    if (link) window.location.hash = link
    n.close()
  }
}
