import type { ChatActivity } from '@/types'

/**
 * RFC-015 §4.3 — descriptor for the "current activity" header shown above the
 * chat timeline while an assistant turn is being built.
 *
 * The selector is intentionally separate from `LiveActivityBar` so it can be
 * unit-tested without a React render. It mirrors the design-doc rule:
 *
 *   currentActivity = activities.findLast(status==='running')
 *                  ?? currentPhase
 *                  ?? default 'Thinking'
 *
 * The store marks `isRunning === true` on tool_start / tool_progress and
 * `isRunning === false` on tool_end (see `chunkToActivity` in
 * `web/src/stores/chat.ts`). Reasoning fragments are fire-and-forget — they
 * carry no completion flag — so the latest reasoning entry is by definition
 * the one being streamed.
 */
export type LiveActivityKind = 'thinking' | 'tool_running' | 'reasoning'

export interface LiveActivityDescriptor {
  kind: LiveActivityKind
  /** Populated only for `tool_running`; used as the label suffix. */
  toolName?: string
}

export function deriveCurrentActivity(
  activities: readonly ChatActivity[] | undefined,
): LiveActivityDescriptor {
  if (!activities || activities.length === 0) {
    return { kind: 'thinking' }
  }
  // Walk backwards so the most recent in-flight activity wins.
  for (let i = activities.length - 1; i >= 0; i--) {
    const a = activities[i]
    if (!a) continue
    if (a.type === 'tool_call' && a.isRunning === true) {
      return { kind: 'tool_running', toolName: a.toolName }
    }
    if (a.type === 'reasoning') {
      return { kind: 'reasoning' }
    }
  }
  return { kind: 'thinking' }
}
