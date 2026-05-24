/**
 * Re-export from the singleton event store.
 *
 * All consumers (useGlobalEvents, /events page, etc.) now share a single
 * SSE connection through the zustand store.
 */
export { useEvents } from '@/stores/events'
