import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useEffect, useRef } from 'react'
import { useEvents } from '@/hooks/use-events'
import { api } from '@/lib/api-client'
import { showDesktopNotification } from '@/lib/desktop-notify'
import { loadNotificationPrefs } from '@/lib/notification-prefs'
import { playNotificationSound } from '@/lib/sound'
import type { NotificationSeverity } from '@/stores/notifications'
import { useNotificationStore } from '@/stores/notifications'
import type { OxiosEvent } from '@/types'

// RFC-024 SP2 (C2 resync): key under which the events store broadcasts a
// `resync` notification. Listeners (this hook, AppLayout) invalidate any
// queries that mirror the dropped stream so the UI re-pulls the same
// state the server would have sent.
const RESYNC_EVENT = 'oxios:resync'

/**
 * Global event listener that converts backend events into notifications.
 *
 * Rules (matching the snake_case wire format emitted by `sanitize_event`):
 * - approval_requested → warning notification, link /approvals
 * - agent_failed       → error notification (infrastructure error)
 * - agent_started      → info notification
 * - agent_stopped      → success (task completed) or warning (evaluation failed)
 *
 * Duplicate suppression:
 * - Same (type, agent_id) within 30s is ignored.
 * - When `agent_failed` fires, a subsequent `agent_stopped(success:false)`
 *   for the same agent within 30s is suppressed (the failure was already
 *   reported via the error notification).
 */
export function useGlobalEvents() {
  const add = useNotificationStore((s) => s.add)
  const { events } = useEvents()
  const seen = useRef<Map<string, number>>(new Map())
  const queryClient = useQueryClient()

  // RFC-024 SP2 (C2): when the SSE bus reports a resync (it lagged and
  // dropped events), the React Query cache is now stale. We invalidate
  // the most user-visible queries so the next render pulls fresh state
  // from the HTTP API instead of showing a half-updated UI.
  useEffect(() => {
    const onResync = () => {
      queryClient.invalidateQueries({ queryKey: ['status'] })
      queryClient.invalidateQueries({ queryKey: ['sessions'] })
      queryClient.invalidateQueries({ queryKey: ['agents'] })
      queryClient.invalidateQueries({ queryKey: ['approvals'] })
    }
    window.addEventListener(RESYNC_EVENT, onResync)
    return () => window.removeEventListener(RESYNC_EVENT, onResync)
  }, [queryClient])

  useEffect(() => {
    for (const event of events) {
      // Dedup: same (type, agent_id) within 30s.
      const key = `${event.type}-${event.agent_id ?? ''}`
      const now = Date.now()
      const lastSeen = seen.current.get(key) ?? 0
      if (now - lastSeen < 30_000) continue
      seen.current.set(key, now)

      // Cross-event dedup: if agent_failed was already emitted for this
      // agent, suppress a trailing agent_stopped(success:false).
      if (event.type === 'agent_stopped') {
        const failedKey = `agent_failed-${event.agent_id ?? ''}`
        const lastFailed = seen.current.get(failedKey) ?? 0
        if (now - lastFailed < 30_000) continue
      }

      const title = eventTitle(event)
      if (!title) continue

      const severity = eventSeverity(event)
      const message = eventMessage(event)
      const link = eventLink(event)

      add({ title, message, severity, link })

      // Desktop notification + sound (controlled by user prefs).
      const prefs = loadNotificationPrefs()
      if (prefs.desktop_notifications_enabled) {
        showDesktopNotification(title, message, link)
      }
      if (prefs.sound_enabled) {
        const soundSeverity = shouldPlaySound(severity, prefs)
        if (soundSeverity) playNotificationSound(soundSeverity)
      }
    }
  }, [events, add])
}

/**
 * Determine if a sound should play for this severity, respecting per-severity prefs.
 */
function shouldPlaySound(
  severity: NotificationSeverity,
  prefs: { complete_sound_enabled: boolean; error_sound_enabled: boolean },
): NotificationSeverity | null {
  switch (severity) {
    case 'success':
      return prefs.complete_sound_enabled ? 'success' : null
    case 'error':
      return prefs.error_sound_enabled ? 'error' : null
    case 'warning':
      // warning (eval failure, approval) is a "negative" event — gated
      // by error_sound_enabled alongside `error` severity.
      return prefs.error_sound_enabled ? 'warning' : null
    case 'info':
      return 'info'
  }
}

/**
 * Poll pending approvals count and emit a notification when new approvals appear.
 */
export function useApprovalWatcher() {
  const add = useNotificationStore((s) => s.add)
  const prevCount = useRef(0)

  const { data } = useQuery({
    queryKey: ['approvals-pending-count'],
    queryFn: async () => {
      const res = await api.get<{ id: string; status: string }[]>('/api/approvals')
      const items = Array.isArray(res) ? res : []
      return items.filter((a) => a.status === 'pending').length
    },
    refetchInterval: 10_000,
  })

  const count = data ?? 0

  useEffect(() => {
    if (count > prevCount.current) {
      add({
        title: 'Approval Required',
        message: `${count - prevCount.current} new request(s) pending`,
        severity: 'warning',
        link: '/approvals',
      })
    }
    prevCount.current = count
  }, [count, add])

  return count
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/**
 * Extract the `success` flag from an agent_stopped event.
 * Defaults to `true` when absent (backward compat with older backends).
 */
function agentStoppedSuccess(e: OxiosEvent): boolean {
  if (e.data && typeof e.data.success === 'boolean') {
    return e.data.success
  }
  return true
}

function eventTitle(e: OxiosEvent): string | null {
  switch (e.type) {
    case 'approval_requested':
      return 'Approval Required'
    case 'agent_failed':
      return 'Agent Failed'
    case 'agent_started':
      return 'Agent Started'
    case 'agent_stopped':
      return agentStoppedSuccess(e) ? 'Task Completed' : 'Task Failed'
    default:
      return null
  }
}

function eventMessage(e: OxiosEvent): string {
  const agent = e.agent_id ? `Agent ${e.agent_id.slice(0, 8)}…` : 'Unknown agent'
  const detail = e.data?.description ?? e.data?.error ?? ''
  return detail ? `${agent}: ${String(detail).slice(0, 100)}` : agent
}

function eventSeverity(e: OxiosEvent): NotificationSeverity {
  switch (e.type) {
    case 'approval_requested':
      return 'warning'
    case 'agent_failed':
      return 'error'
    case 'agent_stopped':
      return agentStoppedSuccess(e) ? 'success' : 'warning'
    default:
      return 'info'
  }
}

function eventLink(e: OxiosEvent): string | undefined {
  switch (e.type) {
    case 'approval_requested':
      return '/approvals'
    case 'agent_failed':
    case 'agent_started':
    case 'agent_stopped':
      return e.agent_id ? '/agents' : undefined
    default:
      return undefined
  }
}
