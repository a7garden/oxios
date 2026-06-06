import { useState } from 'react'
import { Label } from '@/components/ui/label'
import { Select } from '@/components/ui/select'
import { Input } from '@/components/ui/input'
import { Switch } from '@/components/ui/switch'
import { Button } from '@/components/ui/button'
import { X } from 'lucide-react'
import type { RepeatRule } from '@/types/calendar'

interface Props {
  value: RepeatRule | undefined
  onChange: (value: RepeatRule | undefined) => void
}

const FREQUENCY_OPTIONS = [
  { label: '매일', value: 'daily' },
  { label: '매주', value: 'weekly' },
  { label: '매월', value: 'monthly' },
  { label: '매년', value: 'yearly' },
]

const DAY_LABELS = ['일', '월', '화', '수', '목', '금', '토']

export function RepeatEditor({ value, onChange }: Props) {
  const [expanded, setExpanded] = useState(!!value)

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
    const current = value.days ?? []
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
        <Label className="text-sm font-medium">반복</Label>
        <Switch checked={expanded} onCheckedChange={handleToggle} />
      </div>

      {expanded && value && (
        <div className="space-y-3 rounded-md border p-3">
          {/* Frequency */}
          <div className="flex items-center gap-3">
            <Label className="text-sm whitespace-nowrap">빈도</Label>
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
            <Label className="text-sm whitespace-nowrap">간격</Label>
            <Input
              type="number"
              min={1}
              max={99}
              value={value.interval ?? 1}
              onChange={(e) => updateRule({ interval: Math.max(1, Number(e.target.value)) })}
              className="w-20"
            />
            <span className="text-sm text-muted-foreground">회마다</span>
          </div>

          {/* Weekly day selection */}
          {value.frequency === 'weekly' && (
            <div className="flex items-center gap-1">
              {DAY_LABELS.map((label) => {
                const dayKey = ['sun', 'mon', 'tue', 'wed', 'thu', 'fri', 'sat'][DAY_LABELS.indexOf(label)]!
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
            <Label className="text-sm whitespace-nowrap">종료</Label>
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
                placeholder="종료일"
              />
              <span className="text-sm text-muted-foreground">또는</span>
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
                placeholder="횟수"
              />
              <span className="text-sm text-muted-foreground">회</span>
            </div>
          </div>

          {/* Clear */}
          <div className="flex justify-end">
            <Button type="button" variant="ghost" size="sm" onClick={handleClear}>
              <X className="mr-1 h-3 w-3" />
              반복 제거
            </Button>
          </div>
        </div>
      )}
    </div>
  )
}
