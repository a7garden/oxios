import { useEffect, useRef } from 'react'
import { useQuery } from '@tanstack/react-query'
import { useNotificationStore } from '@/stores/notifications'
import { useEvents } from '@/hooks/use-events'
import { api } from '@/lib/api-client'
import type { OxiosEvent } from '@/types'

/**
 * Global event listener that converts backend events into notifications.
 *
 * Rules:
 * - approval.requested → warning notification, link /approvals
 * - agent.failed → error notification
 * - agent.completed → success notification (only if it was running)
 * - agent.started → info notification
 * - Duplicate suppression: same (type, agent_id) within 30s is ignored.
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
      const res = await api.get<
        { id: string; status: string }[]
      >('/api/approvals')
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
    case 'approval.requested':
      return 'Approval Required'
    case 'agent.failed':
      return 'Agent Failed'
    case 'agent.completed':
      return 'Agent Completed'
    case 'agent.started':
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
    case 'approval.requested':
      return 'warning'
    case 'agent.failed':
      return 'error'
    case 'agent.completed':
      return 'success'
    default:
      return 'info'
  }
}

function eventLink(e: OxiosEvent): string | undefined {
  switch (e.type) {
    case 'approval.requested':
      return '/approvals'
    case 'agent.failed':
    case 'agent.completed':
    case 'agent.started':
      return e.agent_id ? `/agents` : undefined
    default:
      return undefined
  }
}
