import { create } from 'zustand'

/**
 * Client-side UI preferences — persisted to localStorage, not backend config.
 *
 * Follows the same pattern as `useThemeStore`: zustand store with manual
 * localStorage persistence.
 */

export type TimeFormat = '12h' | '24h'

interface UiPrefsState {
  /** Clock format preference. */
  timeFormat: TimeFormat
  /** Update the clock format and persist. */
  setTimeFormat: (tf: TimeFormat) => void
}

const STORAGE_KEY = 'oxios-time-format'
const saved = (localStorage.getItem(STORAGE_KEY) as TimeFormat) || '24h'

export const useUiPrefs = create<UiPrefsState>((set) => ({
  timeFormat: saved,
  setTimeFormat: (timeFormat) => {
    localStorage.setItem(STORAGE_KEY, timeFormat)
    set({ timeFormat })
  },
}))

/**
 * Convenience hook: returns `true` for a 12-hour clock, `false` for 24-hour.
 *
 * Use as the `hour12` option in `toLocaleTimeString`:
 * ```ts
 * const hour12 = useHour12()
 * date.toLocaleTimeString(locale, { hour: '2-digit', minute: '2-digit', hour12 })
 * ```
 */
export function useHour12(): boolean {
  return useUiPrefs((s) => s.timeFormat) === '12h'
}
