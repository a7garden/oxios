import { create } from 'zustand'

/**
 * Global Notification Center state.
 *
 * A macOS-style right slide-over that unifies the calendar widget and the
 * notification feed into a single scrolling view. No tabs — the calendar
 * sits at the top, notifications stack below, just like macOS Notification
 * Center. A single menu-bar trigger (date + badge) opens and closes it.
 *
 * Not persisted — open/close is ephemeral UI state.
 */
interface NotificationCenterState {
  /** Whether the slide-over is visible. */
  open: boolean

  /** Open the center. */
  openCenter: () => void
  /** Close the center. */
  closeCenter: () => void
  /** Toggle open/closed. */
  toggleCenter: () => void
}

export const useNotificationCenter = create<NotificationCenterState>((set, get) => ({
  open: false,

  openCenter: () => set({ open: true }),
  closeCenter: () => set({ open: false }),
  toggleCenter: () => set({ open: !get().open }),
}))
