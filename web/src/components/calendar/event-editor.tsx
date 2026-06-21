import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'
import { Textarea } from '@/components/ui/textarea'
import type {
  CalendarEvent,
  CreateEventRequest,
  RepeatRule,
  UpdateEventRequest,
} from '@/types/calendar'
import { ReminderEditor } from './reminder-editor'
import { RepeatEditor } from './repeat-editor'

interface Props {
  open: boolean
  onClose: () => void
  event?: CalendarEvent
  defaultStart?: Date
  onSubmit: (data: CreateEventRequest | UpdateEventRequest) => void
  isLoading?: boolean
}

/** Format a Date to a datetime-local input value string. */
function toDatetimeLocal(date: Date): string {
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(date.getHours())}:${pad(date.getMinutes())}`
}

/** Parse a datetime-local value string back to an ISO string. */
function fromDatetimeLocal(value: string): string {
  return new Date(value).toISOString()
}

/** Default end time = start + 1 hour. */
function defaultEnd(start: Date): Date {
  const d = new Date(start)
  d.setHours(d.getHours() + 1)
  return d
}

export function EventEditor({ open, onClose, event, defaultStart, onSubmit, isLoading }: Props) {
  const { t } = useTranslation()
  const isEdit = !!event

  const [title, setTitle] = useState('')
  const [start, setStart] = useState('')
  const [end, setEnd] = useState('')
  const [allDay, setAllDay] = useState(false)
  const [description, setDescription] = useState('')
  const [location, setLocation] = useState('')
  const [repeat, setRepeat] = useState<RepeatRule | undefined>(undefined)
  const [reminders, setReminders] = useState<number[]>([])

  // Populate form when event changes or modal opens
  useEffect(() => {
    if (event) {
      setTitle(event.title)
      setStart(toDatetimeLocal(new Date(event.start)))
      setEnd(toDatetimeLocal(new Date(event.end)))
      setAllDay(event.all_day ?? false)
      setDescription(event.description ?? '')
      setLocation(event.location ?? '')
      setRepeat(undefined) // rrule is a raw string, not RepeatRule
      setReminders([]) // not stored on event directly
    } else {
      const startDate = defaultStart ?? new Date()
      setTitle('')
      setStart(toDatetimeLocal(startDate))
      setEnd(toDatetimeLocal(defaultEnd(startDate)))
      setAllDay(false)
      setDescription('')
      setLocation('')
      setRepeat(undefined)
      setReminders([])
    }
  }, [event, defaultStart, open])

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()

    if (!title.trim() || !start || !end) return

    if (isEdit) {
      const data: UpdateEventRequest = {
        title: title.trim(),
        start: fromDatetimeLocal(start),
        end: fromDatetimeLocal(end),
        all_day: allDay,
        description: description.trim() || undefined,
        location: location.trim() || undefined,
        repeat: repeat ?? null,
        reminder_minutes: reminders.length > 0 ? reminders : undefined,
      }
      onSubmit(data)
    } else {
      const data: CreateEventRequest = {
        title: title.trim(),
        start: fromDatetimeLocal(start),
        end: fromDatetimeLocal(end),
        all_day: allDay,
        description: description.trim() || undefined,
        location: location.trim() || undefined,
        repeat,
        reminder_minutes: reminders.length > 0 ? reminders : undefined,
      }
      onSubmit(data)
    }
  }

  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-h-[90vh] overflow-y-auto sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>{isEdit ? t('calendar.editEvent') : t('calendar.createEvent')}</DialogTitle>
        </DialogHeader>

        <form onSubmit={handleSubmit} className="space-y-4">
          {/* Title */}
          <div className="space-y-2">
            <Label htmlFor="event-title">{t('calendar.titleLabel')}</Label>
            <Input
              id="event-title"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              placeholder={t('calendar.titlePlaceholder')}
              required
              autoFocus
            />
          </div>

          {/* All day toggle */}
          <div className="flex items-center justify-between">
            <Label className="text-sm font-medium">{t('calendar.allDay')}</Label>
            <Switch checked={allDay} onCheckedChange={setAllDay} />
          </div>

          {/* Start / End */}
          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-2">
              <Label htmlFor="event-start">{t('calendar.start')}</Label>
              <Input
                id="event-start"
                type={allDay ? 'date' : 'datetime-local'}
                value={start}
                onChange={(e) => setStart(e.target.value)}
                required
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="event-end">{t('calendar.end')}</Label>
              <Input
                id="event-end"
                type={allDay ? 'date' : 'datetime-local'}
                value={end}
                onChange={(e) => setEnd(e.target.value)}
                required
              />
            </div>
          </div>

          {/* Location */}
          <div className="space-y-2">
            <Label htmlFor="event-location">{t('calendar.location')}</Label>
            <Input
              id="event-location"
              value={location}
              onChange={(e) => setLocation(e.target.value)}
              placeholder={t('calendar.locationPlaceholder')}
            />
          </div>

          {/* Description */}
          <div className="space-y-2">
            <Label htmlFor="event-description">{t('calendar.description')}</Label>
            <Textarea
              id="event-description"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder={t('calendar.descriptionPlaceholder')}
              rows={3}
            />
          </div>

          {/* Repeat */}
          <RepeatEditor value={repeat} onChange={setRepeat} />

          {/* Reminders */}
          <div className="space-y-2">
            <Label className="text-sm font-medium">{t('calendar.reminder')}</Label>
            <ReminderEditor value={reminders} onChange={setReminders} />
          </div>

          {/* Actions */}
          <DialogFooter>
            <Button type="button" variant="outline" onClick={onClose} disabled={isLoading}>
              {t('calendar.cancel')}
            </Button>
            <Button type="submit" disabled={isLoading || !title.trim() || !start || !end}>
              {isLoading
                ? t('calendar.processing')
                : isEdit
                  ? t('calendar.editEvent')
                  : t('calendar.createEvent')}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
