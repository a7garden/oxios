import { X } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Select } from '@/components/ui/select'
import { Switch } from '@/components/ui/switch'
import type { RepeatRule } from '@/types/calendar'

interface Props {
  value: RepeatRule | undefined
  onChange: (value: RepeatRule | undefined) => void
}

export function RepeatEditor({ value, onChange }: Props) {
  const { t } = useTranslation()
  const [expanded, setExpanded] = useState(!!value)

  const FREQUENCY_OPTIONS = [
    { label: t('calendar.freqDaily'), value: 'daily' },
    { label: t('calendar.freqWeekly'), value: 'weekly' },
    { label: t('calendar.freqMonthly'), value: 'monthly' },
    { label: t('calendar.freqYearly'), value: 'yearly' },
  ]

  const DAY_LABELS = [
    t('calendar.daySun'),
    t('calendar.dayMon'),
    t('calendar.dayTue'),
    t('calendar.dayWed'),
    t('calendar.dayThu'),
    t('calendar.dayFri'),
    t('calendar.daySat'),
  ]

  const handleToggle = (checked: boolean) => {
    setExpanded(checked)
    if (!checked) {
      onChange(undefined)
    } else if (!value) {
      onChange({ frequency: 'daily', interval: 1 })
    }
  }

  const updateRule = (partial: Partial<RepeatRule>) => {
    if (!value) return
    onChange({ ...value, ...partial })
  }

  const toggleDay = (dayLabel: string) => {
    if (!value) return
    const current = Array.isArray(value?.days) ? value.days : []
    const next = current.includes(dayLabel)
      ? current.filter((d) => d !== dayLabel)
      : [...current, dayLabel]
    updateRule({ days: next })
  }

  const handleClear = () => {
    setExpanded(false)
    onChange(undefined)
  }

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <Label className="text-sm font-medium">{t('calendar.repeat')}</Label>
        <Switch checked={expanded} onCheckedChange={handleToggle} />
      </div>

      {expanded && value && (
        <div className="space-y-3 rounded-md border p-3">
          {/* Frequency */}
          <div className="flex items-center gap-3">
            <Label className="text-sm whitespace-nowrap">{t('calendar.frequency')}</Label>
            <Select
              value={value.frequency}
              onValueChange={(v) =>
                updateRule({
                  frequency: v as RepeatRule['frequency'],
                  days: v === 'weekly' ? ['mon'] : undefined,
                })
              }
              options={FREQUENCY_OPTIONS}
              className="flex-1"
            />
          </div>

          {/* Interval */}
          <div className="flex items-center gap-3">
            <Label className="text-sm whitespace-nowrap">{t('calendar.interval')}</Label>
            <Input
              type="number"
              min={1}
              max={99}
              value={value.interval ?? 1}
              onChange={(e) => updateRule({ interval: Math.max(1, Number(e.target.value)) })}
              className="w-20"
            />
            <span className="text-sm text-muted-foreground">{t('calendar.intervalSuffix')}</span>
          </div>

          {/* Weekly day selection */}
          {value.frequency === 'weekly' && (
            <div className="flex items-center gap-1">
              {DAY_LABELS.map((label) => {
                const dayKey = ['sun', 'mon', 'tue', 'wed', 'thu', 'fri', 'sat'][
                  DAY_LABELS.indexOf(label)
                ]!
                const active = (value.days ?? []).includes(dayKey)
                return (
                  <button
                    key={dayKey}
                    type="button"
                    onClick={() => toggleDay(dayKey)}
                    className={`flex h-8 w-8 items-center justify-center rounded-md text-xs font-medium transition-colors ${
                      active
                        ? 'bg-primary text-primary-foreground'
                        : 'bg-muted text-muted-foreground hover:bg-accent'
                    }`}
                  >
                    {label}
                  </button>
                )
              })}
            </div>
          )}

          {/* Until / Count */}
          <div className="flex items-center gap-3">
            <Label className="text-sm whitespace-nowrap">{t('calendar.endCondition')}</Label>
            <div className="flex flex-1 items-center gap-2">
              <Input
                type="date"
                value={value.until ?? ''}
                onChange={(e) =>
                  updateRule({
                    until: e.target.value || undefined,
                    count: e.target.value ? undefined : value.count,
                  })
                }
                className="flex-1"
                placeholder={t('calendar.endDatePlaceholder')}
              />
              <span className="text-sm text-muted-foreground">{t('calendar.or')}</span>
              <Input
                type="number"
                min={1}
                max={999}
                value={value.count ?? ''}
                onChange={(e) =>
                  updateRule({
                    count: e.target.value ? Math.max(1, Number(e.target.value)) : undefined,
                    until: e.target.value ? undefined : value.until,
                  })
                }
                className="w-20"
                placeholder={t('calendar.countPlaceholder')}
              />
              <span className="text-sm text-muted-foreground">{t('calendar.countSuffix')}</span>
            </div>
          </div>

          {/* Clear */}
          <div className="flex justify-end">
            <Button type="button" variant="ghost" size="sm" onClick={handleClear}>
              <X className="mr-1 h-3 w-3" />
              {t('calendar.clearRepeat')}
            </Button>
          </div>
        </div>
      )}
    </div>
  )
}
