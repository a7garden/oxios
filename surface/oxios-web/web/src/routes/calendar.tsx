import { createFileRoute } from '@tanstack/react-router'
import { Plus } from 'lucide-react'
import { useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { CalendarView } from '@/components/calendar/calendar-view'
import { ConflictWarning } from '@/components/calendar/conflict-warning'
import { EventDetail } from '@/components/calendar/event-detail'
import { EventEditor } from '@/components/calendar/event-editor'
import { Button } from '@/components/ui/button'
import {
  useCalendarCreate,
  useCalendarDelete,
  useCalendarEvents,
  useCalendarUpdate,
} from '@/hooks/use-calendar'
import type { CalendarEvent, CreateEventRequest } from '@/types/calendar'

export const Route = createFileRoute('/calendar')({ component: CalendarPage })

function CalendarPage() {
  const { t } = useTranslation()
  const [selectedEvent, setSelectedEvent] = useState<CalendarEvent | null>(null)
  const [editorOpen, setEditorOpen] = useState(false)
  const [editingEvent, setEditingEvent] = useState<CalendarEvent | undefined>()
  const [defaultStart, setDefaultStart] = useState<Date | undefined>()

  // Date range: past 1 month + next 2 months for the calendar view
  const { from, to } = useMemo(() => {
    const now = new Date()
    const from = new Date(now.getFullYear(), now.getMonth() - 1, 1)
    const to = new Date(now.getFullYear(), now.getMonth() + 2, 0)
    return {
      from: from.toISOString(),
      to: new Date(to.getTime() + 86400000).toISOString(),
    }
  }, [])

  const { data, isLoading } = useCalendarEvents(from, to)
  const events = Array.isArray(data?.events) ? data.events : []

  const createMutation = useCalendarCreate()
  const updateMutation = useCalendarUpdate()
  const deleteMutation = useCalendarDelete()

  const handleCreate = (req: CreateEventRequest) => {
    createMutation.mutate(req, {
      onSuccess: () => setEditorOpen(false),
    })
  }

  const handleDelete = () => {
    if (selectedEvent) {
      deleteMutation.mutate(selectedEvent.uid, {
        onSuccess: () => setSelectedEvent(null),
      })
    }
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">{t('calendar.title')}</h1>
          <p className="text-sm text-muted-foreground">{t('calendar.subtitle')}</p>
        </div>
        <Button
          onClick={() => {
            setEditingEvent(undefined)
            setDefaultStart(new Date())
            setEditorOpen(true)
          }}
        >
          <Plus className="h-4 w-4 mr-1" /> {t('calendar.newEvent')}
        </Button>
      </div>

      {createMutation.data?.conflicts && createMutation.data.conflicts.length > 0 && (
        <ConflictWarning conflicts={createMutation.data.conflicts} />
      )}

      {isLoading ? (
        <div className="flex items-center justify-center h-64 text-muted-foreground">
          {t('calendar.loading')}
        </div>
      ) : (
        <CalendarView
          events={events}
          onEventClick={(uid) => {
            const event = events.find((e) => e.uid === uid)
            if (event) setSelectedEvent(event)
          }}
          onDateClick={(date) => {
            setDefaultStart(date)
            setEditingEvent(undefined)
            setEditorOpen(true)
          }}
        />
      )}

      <EventEditor
        open={editorOpen}
        onClose={() => setEditorOpen(false)}
        event={editingEvent}
        defaultStart={defaultStart}
        onSubmit={(data) => {
          if (editingEvent) {
            updateMutation.mutate(
              { uid: editingEvent.uid, ...data },
              {
                onSuccess: () => setEditorOpen(false),
              },
            )
          } else {
            handleCreate(data as CreateEventRequest)
          }
        }}
        isLoading={createMutation.isPending || updateMutation.isPending}
      />

      {selectedEvent && (
        <EventDetail
          event={selectedEvent}
          onEdit={() => {
            setEditingEvent(selectedEvent)
            setEditorOpen(true)
          }}
          onDelete={handleDelete}
          onClose={() => setSelectedEvent(null)}
        />
      )}
    </div>
  )
}
