import { useQuery } from '@tanstack/react-query'
import { useEffect, useRef } from 'react'
import { useEvents } from '@/hooks/use-events'
import { api } from '@/lib/api-client'
import { useNotificationStore } from '@/stores/notifications'
import type { OxiosEvent } from '@/types'

/**
 * Global event listener that converts backend events into notifications.
 *
 * Rules (matching the snake_case wire format emitted by `sanitize_event`):
 * - approval_requested → warning notification, link /approvals
 * - agent_failed → error notification
 * - agent_started → info notification
 * - Duplicate suppression: same (type, agent_id) within 30s is ignored.
 *
 * NOTE: there is no `agent_completed` event on the wire — the backend only
 * emits `agent_stopped`. Successful completion is a `stopped` event with no
 * error payload. Notification rules here intentionally omit it.
 */
export function useGlobalEvents() {
  const add = useNotificationStore((s) => s.add)
  const { events } = useEvents()
  const seen = useRef<Map<string, number>>(new Map())

  useEffect(() => {
    for (const event of events) {
      const key = `${event.type}-${event.agent_id ?? ''}`
      const now = Date.now()
      const lastSeen = seen.current.get(key) ?? 0
      if (now - lastSeen < 30_000) continue // dedup
      seen.current.set(key, now)

      const title = eventTitle(event)
      if (!title) continue

      add({
        title,
        message: eventMessage(event),
        severity: eventSeverity(event),
        link: eventLink(event),
      })
    }
  }, [events, add])
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
    if (count > prevCount.current && prevCount.current > 0) {
      add({
        title: 'New Approval Required',
        message: `${count - prevCount.current} new approval(s) pending`,
        severity: 'warning',
        link: '/approvals',
      })
    }
    prevCount.current = count
  }, [count, add])

  return count
}

// ── Helpers ──────────────────────────────────────────────────────────────────

function eventTitle(e: OxiosEvent): string | null {
  switch (e.type) {
    case 'approval_requested':
      return 'Approval Required'
    case 'agent_failed':
      return 'Agent Failed'
    case 'agent_started':
      return 'Agent Started'
    default:
      return null
  }
}

function eventMessage(e: OxiosEvent): string {
  const agent = e.agent_id ? `Agent ${e.agent_id.slice(0, 8)}…` : 'Unknown agent'
  const detail = e.data?.description ?? e.data?.error ?? ''
  return detail ? `${agent}: ${String(detail).slice(0, 100)}` : agent
}

function eventSeverity(e: OxiosEvent): 'info' | 'warning' | 'error' | 'success' {
  switch (e.type) {
    case 'approval_requested':
      return 'warning'
    case 'agent_failed':
      return 'error'
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
      return e.agent_id ? `/agents` : undefined
    default:
      return undefined
  }
}
