/**
 * Notification preferences stored in localStorage (RFC-028 SP-1e).
 *
 * These are client-side UI preferences — not backend config. They control
 * desktop notifications and sounds independently of the notification store.
 */

export interface NotificationPrefs {
  desktop_notifications_enabled: boolean
  sound_enabled: boolean
  complete_sound_enabled: boolean
  error_sound_enabled: boolean
}

const STORAGE_KEY = 'oxios-notification-prefs'

const DEFAULT_PREFS: NotificationPrefs = {
  desktop_notifications_enabled: false,
  sound_enabled: true,
  complete_sound_enabled: true,
  error_sound_enabled: true,
}

function isNotificationPrefs(value: unknown): value is NotificationPrefs {
  if (!value || typeof value !== 'object') return false
  const obj = value as Record<string, unknown>
  return (
    typeof obj.desktop_notifications_enabled === 'boolean' &&
    typeof obj.sound_enabled === 'boolean' &&
    typeof obj.complete_sound_enabled === 'boolean' &&
    typeof obj.error_sound_enabled === 'boolean'
  )
}

export function loadNotificationPrefs(): NotificationPrefs {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return DEFAULT_PREFS
    const parsed: unknown = JSON.parse(raw)
    return isNotificationPrefs(parsed) ? parsed : DEFAULT_PREFS
  } catch {
    return DEFAULT_PREFS
  }
}

export function saveNotificationPrefs(prefs: NotificationPrefs): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(prefs))
  } catch {
    // localStorage may be full or disabled — fail silently.
  }
}
