import { create } from 'zustand'

/**
 * Global notification severity.
 */
export type NotificationSeverity = 'info' | 'warning' | 'error' | 'success'

/**
 * A single notification entry.
 */
export interface Notification {
  id: string
  /** Short title (e.g. "Approval Required"). */
  title: string
  /** Human-readable description. */
  message?: string
  severity: NotificationSeverity
  /** Route to navigate on click (e.g. "/approvals"). */
  link?: string
  /** ISO timestamp. */
  timestamp: string
  /** Whether the user has dismissed this notification. */
  read: boolean
}

interface NotificationState {
  notifications: Notification[]
  /** Number of unread notifications. */
  unreadCount: number

  /** Add a new notification. */
  add: (n: Omit<Notification, 'id' | 'timestamp' | 'read'>) => void
  /** Mark a single notification as read. */
  markRead: (id: string) => void
  /** Mark all notifications as read. */
  markAllRead: () => void
  /** Dismiss a notification entirely. */
  dismiss: (id: string) => void
  /** Clear all notifications. */
  clear: () => void
}

let _nextId = 0

export const useNotificationStore = create<NotificationState>((set) => ({
  notifications: [],
  unreadCount: 0,

  add(n) {
    const entry: Notification = {
      ...n,
      id: `notif-${++_nextId}-${Date.now()}`,
      timestamp: new Date().toISOString(),
      read: false,
    }
    set((s) => ({
      notifications: [entry, ...s.notifications].slice(0, 50),
      unreadCount: s.unreadCount + 1,
    }))
  },

  markRead(id) {
    set((s) => {
      const wasUnread = s.notifications.find((n) => n.id === id && !n.read)
      return {
        notifications: s.notifications.map((n) =>
          n.id === id ? { ...n, read: true } : n,
        ),
        unreadCount: wasUnread ? Math.max(0, s.unreadCount - 1) : s.unreadCount,
      }
    })
  },

  markAllRead() {
    set((s) => ({
      notifications: s.notifications.map((n) => ({ ...n, read: true })),
      unreadCount: 0,
    }))
  },

  dismiss(id) {
    set((s) => {
      const wasUnread = s.notifications.find((n) => n.id === id && !n.read)
      return {
        notifications: s.notifications.filter((n) => n.id !== id),
        unreadCount: wasUnread ? Math.max(0, s.unreadCount - 1) : s.unreadCount,
      }
    })
  },

  clear() {
    set({ notifications: [], unreadCount: 0 })
  },
}))
