import { create } from 'zustand'
import { persist } from 'zustand/middleware'
import type { ChatActivity, ChatMessage, StreamChunk, ToolCallSummary } from '@/types'
import { useAuthStore } from './auth'

// ---------------------------------------------------------------------------
// Persisted state (survives tab switches)
// ---------------------------------------------------------------------------

interface PersistedState {
  /** Last active session ID (null = no conversation started yet). */
  activeSessionId: string | null
  /** Project ID associated with the active session. */
  activeProjectId: string | null
}

const PERSIST_KEY = 'oxios-chat-persist'

// ---------------------------------------------------------------------------
// Runtime state
// ---------------------------------------------------------------------------

interface ChatRuntimeState {
  /** All messages in the current session (restored from /api/sessions/:id). */
  messages: ChatMessage[]
  isStreaming: boolean
  /** WebSocket connection state. */
  connected: boolean
  /** Queue of messages waiting for WS connection. */
  _sendQueue: string[]
  /** The session ID from the last "done" chunk. */
  _lastDoneSessionId: string | null
  /** The project ID from the last "done" chunk. */
  _lastDoneProjectId: string | null
  /** AI-detected project (Phase 2 stub, always null). */
  detectedProject: import('@/types').Project | null
  /** IDs of dismissed detection badges. */
  dismissedProjectIds: string[]
}

// ---------------------------------------------------------------------------
// Chat store — single source of truth for all chat state
// ---------------------------------------------------------------------------

interface ChatActions {
  /** Start or continue a WebSocket connection. */
  connect: () => Promise<void>
  /** Close the WebSocket and reset all runtime state. */
  disconnect: () => void
  /** Send a message using the active session. */
  sendMessage: (content: string) => void
  /** Load a previous session's message history from the API. */
  loadSession: (sessionId: string) => Promise<void>
  /** Start a fresh session (clears messages). */
  newSession: () => void
  /** Set the active project explicitly. */
  setActiveProject: (projectId: string | null) => void
  /** Set the detected project (Phase 2: called from WS response). */
  setDetectedProject: (project: import('@/types').Project | null) => void
  /** Dismiss a detection badge (don't show again for this project). */
  dismissDetection: (projectId: string) => void
  /** Clear persisted state (e.g. on logout). */
  clearPersist: () => void
  /** Handle an incoming WS chunk. */
  handleChunk: (chunk: StreamChunk) => void
}

export type ChatStore = PersistedState & ChatRuntimeState & ChatActions

// Helper to build a typed chunk from unknown WS data
function parseChunk(raw: unknown): StreamChunk {
  if (typeof raw === 'object' && raw !== null && !Array.isArray(raw)) {
    return raw as StreamChunk
  }
  return { type: 'error', error: 'Malformed chunk' }
}

// Convert an RFC-015 transparency chunk into a ChatActivity entry. Returns
// null for chunk types that should not be persisted on the message (the
// phase chunk is currently informational and grouped via metadata instead).
function chunkToActivity(chunk: StreamChunk): ChatActivity | null {
  const ts = new Date().toISOString()
  const baseId = (id?: string) => `${id ?? crypto.randomUUID()}`
  switch (chunk.type) {
    case 'phase':
      return {
        id: baseId(chunk.phase),
        type: 'phase',
        timestamp: ts,
        phase: chunk.phase,
        status: chunk.status,
        summary: chunk.summary,
      }
    case 'tool_start':
      return {
        id: baseId(chunk.tool_call_id),
        type: 'tool_call',
        timestamp: ts,
        toolName: chunk.tool_name,
        toolCallId: chunk.tool_call_id,
        toolArgs: chunk.tool_args,
        isRunning: true,
      }
    case 'tool_progress':
      return {
        id: baseId(chunk.tool_call_id),
        type: 'tool_call',
        timestamp: ts,
        toolName: chunk.tool_name,
        toolCallId: chunk.tool_call_id,
        progress: chunk.progress,
        isRunning: true,
        ...(chunk.tab_id ? { tabId: chunk.tab_id } : {}),
        ...(chunk.context ? { context: chunk.context } : {}),
      }
    case 'tool_end':
      return {
        id: baseId(chunk.tool_call_id),
        type: 'tool_call',
        timestamp: ts,
        toolName: chunk.tool_name,
        toolCallId: chunk.tool_call_id,
        outputSummary: chunk.output_summary,
        durationMs: chunk.duration_ms,
        isError: chunk.is_error,
        isRunning: false,
      }
    case 'memory':
      return {
        id: baseId(`${chunk.action}-${chunk.query}`),
        type: 'memory',
        timestamp: ts,
        memoryAction: chunk.action,
        query: chunk.query,
        count: chunk.count,
        memorySource: chunk.source,
      }
    case 'reasoning':
      return {
        id: baseId(`reason-${chunk.content?.slice(0, 16)}`),
        type: 'reasoning',
        timestamp: ts,
        content: chunk.content,
        reasoningSource: chunk.source,
      }
    case 'usage':
      return {
        id: baseId(`usage-${Date.now()}`),
        type: 'usage',
        timestamp: ts,
        inputTokens: chunk.input_tokens,
        outputTokens: chunk.output_tokens,
      }
    default:
      return null
  }
}

// Convert a persisted TrajectoryStepRecord (from /api/sessions/:id) into a
// ChatActivity, so the timeline is rendered the same way after reload.
function trajectoryToActivity(step: {
  tool_name: string
  tool_args: unknown
  output_summary: string
  duration_ms: number
  is_error: boolean
  tool_call_id: string
  timestamp: string
  /** Semantic context persisted by the backend (RFC-015). */
  context?: import('@/types').ToolCallContext
}): ChatActivity {
  return {
    id: step.tool_call_id,
    type: 'tool_call',
    timestamp: step.timestamp,
    toolName: step.tool_name,
    toolCallId: step.tool_call_id,
    toolArgs: (step.tool_args as Record<string, unknown> | undefined) ?? undefined,
    outputSummary: step.output_summary,
    durationMs: step.duration_ms,
    isError: step.is_error,
    ...(step.context ? { context: step.context } : {}),
  }
}

// ---------------------------------------------------------------------------
// WS singleton
// ---------------------------------------------------------------------------

let wsInstance: WebSocket | null = null

function getToken(): string {
  return useAuthStore.getState().token || ''
}

async function buildWsUrl(): Promise<string> {
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
  const token = getToken()

  // Try to get a one-time ticket for WS auth
  if (token) {
    try {
      const res = await fetch('/api/chat/ticket', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          Authorization: `Bearer ${token}`,
        },
      })
      if (res.ok) {
        const data = await res.json()
        if (data.ticket) {
          return `${protocol}//${window.location.host}/api/chat/stream?ticket=${encodeURIComponent(data.ticket)}`
        }
      }
    } catch {
      // Ticket endpoint not available, fall back to token
    }
  }

  // Fallback: no auth or ticket endpoint unavailable
  return `${protocol}//${window.location.host}/api/chat/stream`
}

/** Set by the store on connect; used by the WS onmessage handler. */
let chunkHandler: ((chunk: StreamChunk) => void) | null = null

// ---------------------------------------------------------------------------
// Store definition
// ---------------------------------------------------------------------------

export const useChatStore = create<ChatStore>()(
  persist(
    (set, get) => ({
      // ── Persisted ──
      activeSessionId: null,
      activeProjectId: null,

      // ── Runtime ──
      messages: [],
      isStreaming: false,
      connected: false,
      _sendQueue: [],
      _lastDoneSessionId: null,
      _lastDoneProjectId: null,
      detectedProject: null,       // Phase 2 stub: always null
      dismissedProjectIds: [],     // Dismissed detection badges

      // ── Actions ──

      async connect() {
        if (wsInstance && wsInstance.readyState === WebSocket.OPEN) return
        if (typeof window === 'undefined') return

        const url = await buildWsUrl()
        const ws = new WebSocket(url)
        wsInstance = ws
        set({ connected: false, isStreaming: false })

        ws.onopen = () => {
          set({ connected: true })
          // Flush any messages queued while connecting
          const queue = get()._sendQueue
          if (queue.length > 0) {
            set({ _sendQueue: [] })
            for (const msg of queue) {
              get().sendMessage(msg)
            }
          }
        }

        ws.onmessage = (event) => {
          try {
            const raw = JSON.parse(event.data as string)
            const chunk = parseChunk(raw)
            if (chunkHandler) chunkHandler(chunk)
          } catch {
            // Ignore malformed JSON
          }
        }

        ws.onclose = () => {
          wsInstance = null
          set({ connected: false, isStreaming: false })
        }

        ws.onerror = () => {
          ws.close()
        }

        // Wire up chunk handler to current store
        chunkHandler = (chunk: StreamChunk) => get().handleChunk(chunk)
      },

      disconnect() {
        wsInstance?.close()
        wsInstance = null
        chunkHandler = null
        set({
          connected: false,
          isStreaming: false,
        })
      },

      sendMessage(content: string) {
        const { activeSessionId, activeProjectId, connected, connect } = get()

        // Ensure WS is connected first
        if (!connected) {
          connect()
          // Queue the message; WS onopen will flush it via _flushQueue.
          // Avoid infinite retry by queuing once.
          const q = get()._sendQueue
          if (!q.includes(content)) {
            set({ _sendQueue: [...q, content] })
          }
          return
        }

        // Optimistic: add user message immediately
        const userMsg: ChatMessage = {
          id: crypto.randomUUID(),
          role: 'user',
          content,
          timestamp: new Date().toISOString(),
        }
        set((s) => ({ messages: [...s.messages, userMsg], isStreaming: true }))

        // Send via WebSocket with session context
        wsInstance?.send(
          JSON.stringify({
            type: 'message',
            content,
            session_id: activeSessionId ?? '',
            project_ids: activeProjectId ?? '',
          }),
        )
      },

      async loadSession(sessionId: string) {
        if (!sessionId) return
        try {
          const res = await fetch(`/api/sessions/${encodeURIComponent(sessionId)}`, {
            headers: {
              Authorization: `Bearer ${getToken()}`,
              'Content-Type': 'application/json',
            },
          })
          if (!res.ok) return

          const data = await res.json()

          // Reconstruct messages from session history
          const messages: ChatMessage[] = []
          const userMsgs: { content: string; timestamp?: string }[] = data.user_messages ?? []
          const agentMsgs: { content: string; timestamp?: string }[] = data.agent_responses ?? []
          // RFC-015: persisted trajectory for chat transparency replay.
          const trajectorySteps: Array<{
            tool_name: string
            tool_args: unknown
            output_summary: string
            duration_ms: number
            is_error: boolean
            tool_call_id: string
            timestamp: string
            /** Semantic context persisted by the backend (RFC-015). */
            context?: import('@/types').ToolCallContext
          }> = data.trajectory_steps ?? []
          const trajectoryActivities = trajectorySteps.map(trajectoryToActivity)

          const maxLen = Math.max(userMsgs.length, agentMsgs.length)
          for (let i = 0; i < maxLen; i++) {
            const userMsg = userMsgs[i]
            const agentMsg = agentMsgs[i]
            if (userMsg != null) {
              messages.push({
                id: crypto.randomUUID(),
                role: 'user',
                content: userMsg.content,
                timestamp: userMsg.timestamp ?? data.created_at,
              })
            }
            if (agentMsg) {
              // Attach the trajectory to the LAST agent response in the
              // session. Per-turn slicing would need timestamps; full-session
              // attachment is a good enough approximation for the replay view
              // and avoids a more expensive join.
              const isLast = i === maxLen - 1
              messages.push({
                id: crypto.randomUUID(),
                role: 'assistant',
                content: agentMsg.content ?? '',
                timestamp: agentMsg.timestamp ?? data.updated_at,
                ...(isLast && trajectoryActivities.length > 0
                  ? { activities: trajectoryActivities }
                  : null),
              })
            }
          }

          const projectId = data.project_id ?? data.metadata?.project_ids ?? null

          set({
            messages,
            activeSessionId: sessionId,
            activeProjectId: projectId,
            isStreaming: false,
          })
        } catch {
          // Silently fail — network issues shouldn't break the UI
        }
      },

      newSession() {
        set({
          messages: [],
          isStreaming: false,
          activeSessionId: null,
          _lastDoneSessionId: null,
          _lastDoneProjectId: null,
        })
      },

      setActiveProject(projectId: string | null) {
        set({
          activeProjectId: projectId,
          activeSessionId: null,
          messages: [],
          detectedProject: null,   // Clear detection when project changes
        })
      },

      setDetectedProject(project: import('@/types').Project | null) {
        set({ detectedProject: project })
      },

      dismissDetection(projectId: string) {
        set((s) => ({
          dismissedProjectIds: [...s.dismissedProjectIds, projectId],
          detectedProject: s.detectedProject?.id === projectId ? null : s.detectedProject,
        }))
      },

      clearPersist() {
        set({
          activeSessionId: null,
          activeProjectId: null,
          messages: [],
        })
      },

      handleChunk(chunk) {
        switch (chunk.type) {
          case 'token': {
            if (!chunk.content) break
            set((s) => {
              const updated = [...s.messages]
              const last = updated[updated.length - 1]
              if (last?.role === 'assistant') {
                return {
                  messages: [...updated.slice(0, -1), { ...last, content: last.content + chunk.content }],
                }
              }
              return {
                messages: [
                  ...updated,
                  { id: crypto.randomUUID(), role: 'assistant' as const, content: chunk.content ?? '', timestamp: new Date().toISOString() },
                ],
              }
            })
            break
          }

          // ── RFC-015 chat transparency chunks ──
          // These are attached to the most recent assistant message as
          // activity entries. Pre-streaming events (sent before any token)
          // are dropped — they have no message to attach to.
          case 'phase':
          case 'tool_start':
          case 'tool_progress':
          case 'tool_end':
          case 'memory':
          case 'reasoning':
          case 'usage': {
            const activity = chunkToActivity(chunk)
            if (!activity) break
            set((s) => {
              const updated = [...s.messages]
              const last = updated[updated.length - 1]
              if (last?.role !== 'assistant') return s
              const existing = last.activities ?? []

              // For tool_call activities, merge into the matching toolCallId
              // entry instead of appending. `tool_start` creates the
              // placeholder; `tool_end` fills in duration/output/isError.
              if (
                activity.type === 'tool_call' &&
                activity.toolCallId
              ) {
                const idx = existing.findIndex(
                  (a) => a.type === 'tool_call' && a.toolCallId === activity.toolCallId,
                )
                if (idx >= 0) {
                  const prior = existing[idx]!
                  const merged: ChatActivity = {
                    ...prior,
                    ...activity,
                    // Preserve tool_name from the start event — tool_end
                    // re-asserts the same value but be defensive in case
                    // the provider omits it.
                    toolName: prior.toolName ?? activity.toolName,
                  }
                  const newActivities = [...existing]
                  newActivities[idx] = merged
                  return {
                    messages: [
                      ...updated.slice(0, -1),
                      {
                        ...last,
                        activities: newActivities,
                        totalInputTokens: (last.totalInputTokens ?? 0) + (activity.inputTokens ?? 0),
                        totalOutputTokens: (last.totalOutputTokens ?? 0) + (activity.outputTokens ?? 0),
                      },
                    ],
                  }
                }
              }
              // All non-tool_call activities (and unmatched tool_call with
              // a fresh toolCallId) get appended.
              return {
                messages: [
                  ...updated.slice(0, -1),
                  {
                    ...last,
                    activities: [...existing, activity],
                    totalInputTokens: (last.totalInputTokens ?? 0) + (activity.inputTokens ?? 0),
                    totalOutputTokens: (last.totalOutputTokens ?? 0) + (activity.outputTokens ?? 0),
                  },
                ],
              }
            })
            break
          }

          case 'done': {
            const sid = chunk.session_id ?? null
            const vid = chunk.project_id ?? null
            const toolCalls = chunk.tool_calls ?? []
            const phase = chunk.phase
            const evaluationPassed = chunk.evaluation_passed === 'true' || chunk.evaluation_passed === 'True'
            const seedId = chunk.seed_id
            const durationMs = chunk.duration_ms

            // Insert tool call messages and attach metadata
            set((s) => {
              const updated = [...s.messages]
              const toolMessages: ChatMessage[] = (Array.isArray(toolCalls) ? toolCalls : []).map(
                (tc: ToolCallSummary) => ({
                  id: crypto.randomUUID(),
                  role: 'tool' as const,
                  content: '',
                  toolName: tc.tool_name,
                  toolArgs: typeof tc.input === 'string' ? undefined : JSON.parse(tc.input),
                  toolResult: tc.output,
                  toolDurationMs: tc.duration_ms,
                  timestamp: new Date().toISOString(),
                })
              )

              // Find last assistant message and attach metadata
              const lastAssistantIdx = [...updated].reverse().findIndex((m) => m.role === 'assistant')
              if (lastAssistantIdx >= 0) {
                const idx = updated.length - 1 - lastAssistantIdx
                const target = updated[idx]
                if (target) {
                  updated[idx] = {
                    ...target,
                    id: target.id ?? crypto.randomUUID(),
                    metadata: {
                      phase,
                      evaluation_passed: evaluationPassed,
                      seed_id: seedId,
                      duration_ms: durationMs,
                      tool_calls: Array.isArray(toolCalls) ? toolCalls : [],
                    },
                  }
                }
              }

              return { messages: [...updated, ...toolMessages], isStreaming: false }
            })

            if (sid) {
              set({ _lastDoneSessionId: sid, activeSessionId: sid })
            }
            if (vid) {
              set({ activeProjectId: vid, _lastDoneProjectId: vid })
            }
            break
          }
          case 'error': {
            set({ isStreaming: false })
            // Don't inline error into message — will be shown as toast
            break
          }
        }
      },
    }),
    {
      name: PERSIST_KEY,
      partialize: (state): PersistedState => ({
        activeSessionId: state.activeSessionId,
        activeProjectId: state.activeProjectId,
      }),
      onRehydrateStorage: () => (state) => {
        if (!state) return
        // After rehydration, if there's an active session, load its history.
        // Note: WS auto-connect moved to ChatPage component — don't connect
        // from every page load, only when the chat route is active.
        if (state.activeSessionId) {
          state.loadSession(state.activeSessionId)
        }
      },
    },
  ),
)