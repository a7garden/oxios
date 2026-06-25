import { create } from 'zustand'

/**
 * Active tab in the Notification Center slide-over.
 */
export type CenterTab = 'schedule' | 'notifications'

/**
 * Global Notification Center state.
 *
 * The center is a macOS-style right slide-over that unifies the schedule
 * (calendar) and the notification feed behind two tabs. Both the bell and the
 * calendar header icon open the same panel, each defaulting to its own tab.
 *
 * Not persisted — open/close is ephemeral UI state.
 */
interface NotificationCenterState {
  /** Whether the slide-over is visible. */
  open: boolean
  /** Which tab is active. */
  activeTab: CenterTab

  /** Open the center, optionally switching to a specific tab. */
  openCenter: (tab?: CenterTab) => void
  /** Close the center. */
  closeCenter: () => void
  /** Toggle open/closed. Toggling the active tab while open closes it. */
  toggleCenter: (tab?: CenterTab) => void
  /** Switch the active tab (does not change open state). */
  setTab: (tab: CenterTab) => void
}

export const useNotificationCenter = create<NotificationCenterState>((set, get) => ({
  open: false,
  activeTab: 'schedule',

  openCenter: (tab) => set({ open: true, ...(tab ? { activeTab: tab } : {}) }),
  closeCenter: () => set({ open: false }),
  toggleCenter: (tab) => {
    const { open, activeTab } = get()
    // Clicking the trigger for the currently-active tab while open acts as a
    // close — matches macOS menu-bar behavior where the active icon toggles.
    if (open && (!tab || tab === activeTab)) {
      set({ open: false })
    } else {
      set({ open: true, ...(tab ? { activeTab: tab } : {}) })
    }
  },
  setTab: (tab) => set({ activeTab: tab }),
}))
