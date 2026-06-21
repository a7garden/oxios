import { X } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'

interface Props {
  value: number[]
  onChange: (value: number[]) => void
}

export function ReminderEditor({ value, onChange }: Props) {
  const { t } = useTranslation()

  const PRESETS = [
    { label: t('calendar.reminder5min'), minutes: 5 },
    { label: t('calendar.reminder15min'), minutes: 15 },
    { label: t('calendar.reminder30min'), minutes: 30 },
    { label: t('calendar.reminder1hour'), minutes: 60 },
    { label: t('calendar.reminder1day'), minutes: 1440 },
  ]

  function formatReminder(minutes: number): string {
    if (minutes < 60) return `${minutes}${t('calendar.minutesBefore')}`
    if (minutes < 1440) return `${Math.floor(minutes / 60)}${t('calendar.hoursBefore')}`
    return `${Math.floor(minutes / 1440)}${t('calendar.daysBefore')}`
  }

  const [customMinutes, setCustomMinutes] = useState<number>(10)

  const addReminder = (minutes: number) => {
    if (!value.includes(minutes)) {
      onChange([...value, minutes].sort((a, b) => a - b))
    }
  }

  const removeReminder = (minutes: number) => {
    onChange(value.filter((m) => m !== minutes))
  }

  return (
    <div className="space-y-3">
      {/* Existing reminders */}
      {value.length > 0 && (
        <div className="flex flex-wrap gap-2">
          {value.map((minutes) => (
            <Badge key={minutes} variant="secondary" className="cursor-pointer gap-1 pr-1">
              {formatReminder(minutes)}
              <button
                type="button"
                onClick={() => removeReminder(minutes)}
                className="ml-1 rounded-full p-0.5 hover:bg-muted-foreground/20"
              >
                <X className="h-3 w-3" />
              </button>
            </Badge>
          ))}
        </div>
      )}

      {/* Preset buttons */}
      <div className="flex flex-wrap gap-2">
        {PRESETS.map(({ label, minutes }) => (
          <Button
            key={minutes}
            type="button"
            variant={value.includes(minutes) ? 'default' : 'outline'}
            size="sm"
            onClick={() =>
              value.includes(minutes) ? removeReminder(minutes) : addReminder(minutes)
            }
          >
            {label}
          </Button>
        ))}
      </div>

      {/* Custom input */}
      <div className="flex items-center gap-2">
        <Input
          type="number"
          min={1}
          max={10080}
          value={customMinutes}
          onChange={(e) => setCustomMinutes(Math.max(1, Number(e.target.value)))}
          className="w-20"
        />
        <span className="text-sm text-muted-foreground">{t('calendar.minutesBefore')}</span>
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={() => addReminder(customMinutes)}
          disabled={value.includes(customMinutes)}
        >
          {t('calendar.add')}
        </Button>
      </div>
    </div>
  )
}
