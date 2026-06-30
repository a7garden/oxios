import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api } from '@/lib/api-client'
import type {
  CalendarEvent,
  CreateEventRequest,
  CreateEventResult,
  EventsResponse,
  FreeBusyResponse,
  UpdateEventRequest,
  UpdateEventResult,
} from '@/types/calendar'

export function useCalendarEvents(from: string, to: string) {
  return useQuery({
    queryKey: ['calendar', 'events', from, to],
    queryFn: () => api.get<EventsResponse>('/api/calendar/events', { from, to }),
  })
}

export function useCalendarEvent(uid: string) {
  return useQuery({
    queryKey: ['calendar', 'event', uid],
    queryFn: () => api.get<CalendarEvent>(`/api/calendar/events/${uid}`),
    enabled: !!uid,
  })
}

export function useCalendarSearch(query: string) {
  return useQuery({
    queryKey: ['calendar', 'search', query],
    queryFn: () => api.get<EventsResponse>('/api/calendar/search', { q: query }),
    enabled: !!query,
  })
}

export function useCalendarByNote(path: string | null) {
  return useQuery({
    queryKey: ['calendar', 'by-note', path],
    queryFn: () => api.get<EventsResponse>('/api/calendar/by-note', { path: path! }),
    enabled: !!path,
  })
}

export function useCalendarFreeBusy(from: string, to: string) {
  return useQuery({
    queryKey: ['calendar', 'freebusy', from, to],
    queryFn: () => api.get<FreeBusyResponse>('/api/calendar/freebusy', { from, to }),
  })
}

export function useCalendarCreate() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (body: CreateEventRequest) =>
      api.post<CreateEventResult>('/api/calendar/events', body),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['calendar'] }),
  })
}

export function useCalendarUpdate() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ uid, ...body }: { uid: string } & UpdateEventRequest) =>
      api.put<UpdateEventResult>(`/api/calendar/events/${uid}`, body),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['calendar'] }),
  })
}

export function useCalendarDelete() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (uid: string) => api.delete(`/api/calendar/events/${uid}`),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['calendar'] }),
  })
}
