/** Calendar types aligned with backend oxios-calendar crate. */

/** A calendar event. */
export interface CalendarEvent {
  uid: string
  title: string
  start: string // ISO 8601
  end: string // ISO 8601
  all_day: boolean
  description: string | null
  location: string | null
  rrule: string | null
  status: string
  source: 'agent' | 'user' | 'cron'
  filename: string
}

/** Repeat rule for recurring events. */
export interface RepeatRule {
  frequency: 'daily' | 'weekly' | 'monthly' | 'yearly'
  days?: string[]
  interval?: number
  until?: string
  count?: number
}

/** Request body for creating a new event. */
export interface CreateEventRequest {
  title: string
  start: string
  end: string
  all_day?: boolean
  description?: string
  location?: string
  repeat?: RepeatRule
  reminder_minutes?: number[]
}

/** Request body for updating an existing event. */
export interface UpdateEventRequest {
  title?: string
  start?: string
  end?: string
  all_day?: boolean
  description?: string | null
  location?: string | null
  repeat?: RepeatRule | null
  reminder_minutes?: number[]
}

/** Response for listing events. */
export interface EventsResponse {
  events: CalendarEvent[]
}

/** Result of creating an event. */
export interface CreateEventResult {
  uid: string
  status: string
  conflicts: EventConflict[]
  file: string
}

/** Result of updating an event. */
export interface UpdateEventResult {
  uid: string
  status: string
  conflicts: EventConflict[]
}

/** Free/busy slot. */
export interface FreeBusySlot {
  start: string
  end: string
  busy: boolean
}

/** Free/busy response. */
export interface FreeBusyResponse {
  slots: FreeBusySlot[]
}

/** Conflict between two calendar events. */
export interface EventConflict {
  uid: string
  title: string
  overlap_minutes: number
}
