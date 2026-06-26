/**
 * Notification preferences section (RFC-028 SP-1e).
 *
 * Client-side UI preferences stored in localStorage — not backend config.
 * Controls desktop notifications, sounds, and clock format. Uses the
 * declarative settings visual model (SectionCard) but saves directly to
 * localStorage without going through the config PATCH flow.
 */
import { Bell } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import {
  loadNotificationPrefs,
  type NotificationPrefs,
  saveNotificationPrefs,
} from '@/lib/notification-prefs'
import { type TimeFormat, useUiPrefs } from '@/stores/ui-prefs'
import { SectionCard } from './section-card'

interface PrefToggle {
  key: keyof NotificationPrefs
  labelKey: string
  descKey: string
}

const PREF_TOGGLES: PrefToggle[] = [
  {
    key: 'desktop_notifications_enabled',
    labelKey: 'settings.notifDesktopEnabled',
    descKey: 'settings.notifDesktopEnabledDesc',
  },
  {
    key: 'sound_enabled',
    labelKey: 'settings.notifSoundEnabled',
    descKey: 'settings.notifSoundEnabledDesc',
  },
  {
    key: 'complete_sound_enabled',
    labelKey: 'settings.notifCompleteSound',
    descKey: 'settings.notifCompleteSoundDesc',
  },
  {
    key: 'error_sound_enabled',
    labelKey: 'settings.notifErrorSound',
    descKey: 'settings.notifErrorSoundDesc',
  },
]

export function NotificationSectionCard() {
  const { t } = useTranslation()
  const [prefs, setPrefs] = useState<NotificationPrefs>(loadNotificationPrefs)
  const timeFormat = useUiPrefs((s) => s.timeFormat)
  const setTimeFormat = useUiPrefs((s) => s.setTimeFormat)

  const update = (key: keyof NotificationPrefs, value: boolean) => {
    const next = { ...prefs, [key]: value }
    setPrefs(next)
    saveNotificationPrefs(next)
  }

  return (
    <SectionCard
      title={t('settings.sectionNotifications', 'Notifications')}
      description={t(
        'settings.notificationsDescription',
        'Desktop notifications and sound preferences',
      )}
      icon={<Bell className="h-3.5 w-3.5" />}
      sectionId="notifications"
      fieldCount={PREF_TOGGLES.length + 1}
      modified={false}
    >
      <div className="space-y-4">
        {PREF_TOGGLES.map((toggle) => (
          <div key={toggle.key} className="flex items-start justify-between gap-4">
            <div className="space-y-0.5">
              <label className="text-sm font-medium leading-none">
                {t(toggle.labelKey, toggle.key)}
              </label>
              <p className="text-xs text-muted-foreground">{t(toggle.descKey, '')}</p>
            </div>
            <button
              type="button"
              role="switch"
              aria-checked={prefs[toggle.key]}
              onClick={() => update(toggle.key, !prefs[toggle.key])}
              className={`relative inline-flex h-5 w-9 shrink-0 items-center rounded-full transition-colors ${
                prefs[toggle.key] ? 'bg-primary' : 'bg-input'
              }`}
            >
              <span
                className={`inline-block h-4 w-4 transform rounded-full bg-background shadow-sm transition-transform ${
                  prefs[toggle.key] ? 'translate-x-4' : 'translate-x-0.5'
                }`}
              />
            </button>
          </div>
        ))}
      </div>

      {/* Time format preference */}
      <div className="mt-4 border-t pt-4">
        <div className="flex items-center justify-between gap-4">
          <div className="space-y-0.5">
            <label className="text-sm font-medium leading-none">
              {t('settings.timeFormat', 'Time Format')}
            </label>
            <p className="text-xs text-muted-foreground">
              {t('settings.timeFormatDesc', '12-hour or 24-hour clock display')}
            </p>
          </div>
          <div className="flex shrink-0 rounded-lg bg-muted p-0.5">
            {(['12h', '24h'] as TimeFormat[]).map((fmt) => (
              <button
                key={fmt}
                type="button"
                onClick={() => setTimeFormat(fmt)}
                className={`rounded-md px-3 py-1 text-xs font-medium transition-colors ${
                  timeFormat === fmt
                    ? 'bg-background text-foreground shadow-sm'
                    : 'text-muted-foreground hover:text-foreground'
                }`}
              >
                {fmt === '12h'
                  ? t('settings.format12h', '12-hour')
                  : t('settings.format24h', '24-hour')}
              </button>
            ))}
          </div>
        </div>
      </div>
    </SectionCard>
  )
}
