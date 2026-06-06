import { X } from 'lucide-react'
import { useState } from 'react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'

interface Props {
  value: number[]
  onChange: (value: number[]) => void
}

const PRESETS = [
  { label: '5분', minutes: 5 },
  { label: '15분', minutes: 15 },
  { label: '30분', minutes: 30 },
  { label: '1시간', minutes: 60 },
  { label: '1일', minutes: 1440 },
]

function formatReminder(minutes: number): string {
  if (minutes < 60) return `${minutes}분 전`
  if (minutes < 1440) return `${Math.floor(minutes / 60)}시간 전`
  return `${Math.floor(minutes / 1440)}일 전`
}

export function ReminderEditor({ value, onChange }: Props) {
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
        <span className="text-sm text-muted-foreground">분 전</span>
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={() => addReminder(customMinutes)}
          disabled={value.includes(customMinutes)}
        >
          추가
        </Button>
      </div>
    </div>
  )
}
