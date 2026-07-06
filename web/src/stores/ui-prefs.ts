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
  /** Whether to surface advanced settings in the UI. */
  showAdvancedSettings: boolean
  /** Update the clock format and persist. */
  setTimeFormat: (tf: TimeFormat) => void
  /** Update the advanced settings visibility and persist. */
  setShowAdvancedSettings: (v: boolean) => void
}

const TIME_FORMAT_STORAGE_KEY = 'oxios-time-format'
const SHOW_ADVANCED_SETTINGS_STORAGE_KEY = 'oxios-show-advanced-settings'
const savedTimeFormat = (localStorage.getItem(TIME_FORMAT_STORAGE_KEY) as TimeFormat) || '24h'
const savedShowAdvancedSettings =
  localStorage.getItem(SHOW_ADVANCED_SETTINGS_STORAGE_KEY) === 'true'

export const useUiPrefs = create<UiPrefsState>((set) => ({
  timeFormat: savedTimeFormat,
  showAdvancedSettings: savedShowAdvancedSettings,
  setTimeFormat: (timeFormat) => {
    localStorage.setItem(TIME_FORMAT_STORAGE_KEY, timeFormat)
    set({ timeFormat })
  },
  setShowAdvancedSettings: (showAdvancedSettings) => {
    localStorage.setItem(SHOW_ADVANCED_SETTINGS_STORAGE_KEY, String(showAdvancedSettings))
    set({ showAdvancedSettings })
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
