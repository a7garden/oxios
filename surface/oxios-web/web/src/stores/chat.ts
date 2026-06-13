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
  /** Active structured interview questions (null = no interview active). */
  activeInterview: import('@/types').InterviewQuestion[] | null
  /** Active tool approval request awaiting user response (RFC-017). */
  activeToolApproval: {
    id: string
    toolName: string
    reason: string
  } | null
  /** Interview round number. */
  interviewRound: number
  /** Interview ambiguity score. */
  interviewAmbiguity: number
  /** Per-session spec mode map: sessionId → true (ouroboros) / false (chat). */
  specModes: Record<string, boolean>
  /** Effective spec mode for the current active session. */
  specMode: boolean

  // ── WebSocket lifecycle (encapsulated, not persisted) ──
  /** WebSocket instance managed by the store. */
  _ws: WebSocket | null
  /** Reconnect timer (exponential backoff). */
  _reconnectTimer: ReturnType<typeof setTimeout> | null
  /** Reconnect attempt counter. */
  _reconnectAttempts: number
}

// ---------------------------------------------------------------------------
// Chat store — single source of truth for all chat state
// ---------------------------------------------------------------------------

interface ChatActions {
  /** Start or continue a WebSocket connection. */
  connect: () => Promise<void>
  /** Close the WebSocket and reset connection state. */
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
  /** Submit interview answers and send them as a message. */
  submitInterviewResponse: (answers: import('@/types').InterviewAnswer[]) => void
  /** Resolve a pending tool approval (RFC-017). */
  resolveToolApproval: (id: string, approved: boolean) => Promise<void>
  /** Handle an incoming WS chunk. */
  handleChunk: (chunk: StreamChunk) => void
  /** Toggle spec (Ouroboros) mode for the current session. */
  toggleSpecMode: () => void
}

export type ChatStore = PersistedState & ChatRuntimeState & ChatActions

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function parseChunk(raw: unknown): StreamChunk {
  if (typeof raw === 'object' && raw !== null && !Array.isArray(raw)) {
    return raw as StreamChunk
  }
  return { type: 'error', error: 'Malformed chunk' }
}

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

function trajectoryToActivity(step: {
  tool_name: string
  tool_args: unknown
  output_summary: string
  duration_ms: number
  is_error: boolean
  tool_call_id: string
  timestamp: string
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

function getToken(): string {
  return useAuthStore.getState().token || ''
}

async function buildWsUrl(): Promise<string> {
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
  const token = getToken()

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

  return `${protocol}//${window.location.host}/api/chat/stream`
}

/** Max reconnect attempts before giving up. */
const MAX_RECONNECT_ATTEMPTS = 5

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
      detectedProject: null,
      dismissedProjectIds: [],
      activeInterview: null,
      interviewRound: 0,
      interviewAmbiguity: 0,
      activeToolApproval: null,
      specModes: {},
      specMode: false,
      // WebSocket lifecycle
      _ws: null,
      _reconnectTimer: null,
      _reconnectAttempts: 0,

      // ── Actions ──

      async connect() {
        const currentWs = get()._ws

        // Already connected — nothing to do.
        if (currentWs && currentWs.readyState === WebSocket.OPEN) return
        if (typeof window === 'undefined') return

        // Tear down any previous connection (stale or connecting).
        if (currentWs) {
          currentWs.onopen = null
          currentWs.onmessage = null
          currentWs.onclose = null
          currentWs.onerror = null
          if (
            currentWs.readyState === WebSocket.OPEN ||
            currentWs.readyState === WebSocket.CONNECTING
          ) {
            currentWs.close()
          }
        }

        // Clear any pending reconnect timer.
        const prevTimer = get()._reconnectTimer
        if (prevTimer) {
          clearTimeout(prevTimer)
          set({ _reconnectTimer: null })
        }

        const url = await buildWsUrl()
        const ws = new WebSocket(url)

        // Store reference so stale-checks work.
        set({ _ws: ws, connected: false, isStreaming: false })

        ws.onopen = () => {
          // If another connect() replaced this ws, ignore.
          if (get()._ws !== ws) return
          set({ connected: true, _reconnectAttempts: 0 })
          // Flush queued messages.
          const queue = get()._sendQueue
          if (queue.length > 0) {
            set({ _sendQueue: [] })
            for (const msg of queue) {
              get().sendMessage(msg)
            }
          }
        }

        ws.onmessage = (event) => {
          // Stale connection — ignore.
          if (get()._ws !== ws) return
          try {
            const raw = JSON.parse(event.data as string)
            const chunk = parseChunk(raw)
            get().handleChunk(chunk)
          } catch {
            // Ignore malformed JSON
          }
        }

        ws.onclose = () => {
          // Another connect() already replaced this ws — do nothing.
          if (get()._ws !== ws) return

          set({ connected: false, isStreaming: false, _ws: null })

          // Auto-reconnect with exponential backoff.
          const attempt = get()._reconnectAttempts
          if (attempt >= MAX_RECONNECT_ATTEMPTS) return

          const delay = 1000 * 2 ** attempt
          const timer = setTimeout(() => {
            set({ _reconnectTimer: null })
            // Only reconnect if no new connection was established in the meantime.
            if (get()._ws === null) {
              set({ _reconnectAttempts: attempt + 1 })
              get().connect()
            }
          }, delay)
          set({ _reconnectTimer: timer })
        }

        ws.onerror = () => {
          if (get()._ws !== ws) return
          ws.close()
        }
      },

      disconnect() {
        const { _ws, _reconnectTimer } = get()

        // Stop any pending reconnect.
        if (_reconnectTimer) clearTimeout(_reconnectTimer)

        if (_ws) {
          // Detach handlers before closing to prevent onclose from
          // triggering auto-reconnect.
          _ws.onopen = null
          _ws.onmessage = null
          _ws.onclose = null
          _ws.onerror = null
          _ws.close()
        }

        set({
          connected: false,
          isStreaming: false,
          _ws: null,
          _reconnectTimer: null,
          _reconnectAttempts: 0,
        })
      },

      sendMessage(content: string) {
        const { activeSessionId, activeProjectId, connected, connect, _ws } = get()

        // Ensure WS is connected first
        if (!connected || !_ws || _ws.readyState !== WebSocket.OPEN) {
          connect()
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

        // Send via WebSocket with session context and mode
        const payload: Record<string, unknown> = {
          type: 'message',
          content,
          session_id: activeSessionId ?? '',
          project_ids: activeProjectId ?? '',
        }
        const effectiveSpecMode = get().specMode
        if (effectiveSpecMode) {
          payload.mode = 'spec'
        }
        _ws.send(JSON.stringify(payload))
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
          const agentMsgs: Array<{
            content: string
            timestamp?: string
            trajectory_range?: { start: number; end: number }
          }> = data.agent_responses ?? []
          const trajectorySteps: Array<{
            tool_name: string
            tool_args: unknown
            output_summary: string
            duration_ms: number
            is_error: boolean
            tool_call_id: string
            timestamp: string
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
              // Per-turn trajectory mapping: if the backend provides
              // trajectory_range on each AgentResponse, extract only the
              // relevant slice. Fallback: attach all to last response.
              const range = agentMsg.trajectory_range
              let activitiesForThisTurn: ChatActivity[] | undefined
              if (range && trajectoryActivities.length > 0) {
                activitiesForThisTurn = trajectoryActivities.slice(range.start, range.end)
                if (activitiesForThisTurn.length === 0) activitiesForThisTurn = undefined
              } else {
                // Fallback for sessions saved before trajectory_range was added:
                // attach all activities to the last assistant message.
                const isLast = i === maxLen - 1
                if (isLast && trajectoryActivities.length > 0) {
                  activitiesForThisTurn = trajectoryActivities
                }
              }
              messages.push({
                id: crypto.randomUUID(),
                role: 'assistant',
                content: agentMsg.content ?? '',
                timestamp: agentMsg.timestamp ?? data.updated_at,
                ...(activitiesForThisTurn ? { activities: activitiesForThisTurn } : null),
              })
            }
          }

          const projectId = data.project_id ?? data.metadata?.project_ids ?? null
          // Restore spec mode from session metadata (persisted by backend)
          const storedMode = data.metadata?.mode
          const isSpec = storedMode === 'spec' || storedMode === 'ouroboros'
          const updatedSpecModes = { ...get().specModes, [sessionId]: isSpec }

          set({
            messages,
            activeSessionId: sessionId,
            activeProjectId: projectId,
            isStreaming: false,
            specMode: isSpec,
            specModes: updatedSpecModes,
          })
        } catch {
          // Silently fail — network issues shouldn't break the UI
        }
      },

      newSession() {
        set(() => ({
          messages: [],
          isStreaming: false,
          activeSessionId: null,
          _lastDoneSessionId: null,
          _lastDoneProjectId: null,
          activeInterview: null,
          interviewRound: 0,
          interviewAmbiguity: 0,
          specMode: false,
          // Keep specModes for other sessions, just reset active display
        }))
      },

      setActiveProject(projectId: string | null) {
        set({
          activeProjectId: projectId,
          activeSessionId: null,
          messages: [],
          detectedProject: null,
          specMode: false,
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
          activeInterview: null,
          interviewRound: 0,
          interviewAmbiguity: 0,
          activeToolApproval: null,
          specModes: {},
          specMode: false,
        })
      },

      submitInterviewResponse(answers: import('@/types').InterviewAnswer[]) {
        const { _ws, activeInterview, activeSessionId, activeProjectId, interviewRound } = get()
        if (!activeInterview) return

        // Build answer summary for user message bubble
        const answerParts = answers
          .filter((a) => a.value.trim())
          .map((a) => {
            const q = activeInterview.find((q) => q.id === a.question_id)
            return q ? `${q.text}\n→ ${a.value}` : a.value
          })
        const answerText = answerParts.join('\n\n')

        // Persist interview questions as an assistant message BEFORE
        // the user's answer, so the Q&A exchange remains in chat history.
        const interviewMsg: ChatMessage = {
          id: crypto.randomUUID(),
          role: 'assistant',
          content: '',
          timestamp: new Date().toISOString(),
          metadata: {
            phase: 'interview',
            tool_calls: [],
          },
          _interviewQuestions: activeInterview,
          _interviewRound: interviewRound,
        }

        // Send via WebSocket as interview_response
        if (_ws && _ws.readyState === WebSocket.OPEN) {
          _ws.send(
            JSON.stringify({
              type: 'interview_response',
              session_id: activeSessionId ?? '',
              project_id: activeProjectId ?? '',
              answers,
              text: answerText,
            }),
          )
        }

        // Add user message showing their answers
        const userMsg: ChatMessage = {
          id: crypto.randomUUID(),
          role: 'user',
          content: answerText || answers.map((a) => a.value).join(', '),
          timestamp: new Date().toISOString(),
        }

        set((s) => ({
          messages: [...s.messages, interviewMsg, userMsg],
          activeInterview: null,
          interviewRound: 0,
          interviewAmbiguity: 0,
          isStreaming: true,
        }))
      },

      async resolveToolApproval(id: string, approved: boolean) {
        const { activeToolApproval } = get()
        if (!activeToolApproval || activeToolApproval.id !== id) return
        set({ activeToolApproval: null, isStreaming: true })
        try {
          const token = useAuthStore.getState().token
          const res = await fetch(`/api/chat/tool-approval/${encodeURIComponent(id)}/respond`, {
            method: 'POST',
            headers: {
              'Content-Type': 'application/json',
              ...(token ? { Authorization: `Bearer ${token}` } : {}),
            },
            body: JSON.stringify({ approved }),
          })
          if (!res.ok) {
            const err = await res.text().catch(() => 'unknown error')
            throw new Error(`HTTP ${res.status}: ${err}`)
          }
        } catch (e) {
          set({ activeToolApproval, isStreaming: false })
          throw e
        }
      },

      toggleSpecMode() {
        set((s) => {
          const next = !s.specMode
          const sid = s.activeSessionId
          // Persist to per-session map if we have a session ID
          const nextModes = sid ? { ...s.specModes, [sid]: next } : s.specModes
          return { specMode: next, specModes: nextModes }
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
                  messages: [
                    ...updated.slice(0, -1),
                    { ...last, content: last.content + chunk.content },
                  ],
                }
              }
              return {
                messages: [
                  ...updated,
                  {
                    id: crypto.randomUUID(),
                    role: 'assistant' as const,
                    content: chunk.content ?? '',
                    timestamp: new Date().toISOString(),
                  },
                ],
              }
            })
            break
          }

          // ── RFC-015 chat transparency chunks ──
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

              // If the last message is not an assistant message (e.g. the user
              // just submitted an interview response and tool events arrive
              // before the first token chunk), create a placeholder assistant
              // message so the activity timeline has somewhere to attach to.
              // The token chunk will later fill in the content.
              if (last?.role !== 'assistant') {
                const placeholder: ChatMessage = {
                  id: crypto.randomUUID(),
                  role: 'assistant',
                  content: '',
                  timestamp: new Date().toISOString(),
                  activities: [activity],
                }
                return { messages: [...updated, placeholder] }
              }

              const existing = last.activities ?? []

              if (activity.type === 'tool_call' && activity.toolCallId) {
                const idx = existing.findIndex(
                  (a) => a.type === 'tool_call' && a.toolCallId === activity.toolCallId,
                )
                if (idx >= 0) {
                  const prior = existing[idx]!
                  const merged: ChatActivity = {
                    ...prior,
                    ...activity,
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
                        totalInputTokens:
                          (last.totalInputTokens ?? 0) + (activity.inputTokens ?? 0),
                        totalOutputTokens:
                          (last.totalOutputTokens ?? 0) + (activity.outputTokens ?? 0),
                      },
                    ],
                  }
                }
              }
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

          case 'interview': {
            if (chunk.questions && chunk.questions.length > 0) {
              set({
                activeInterview: chunk.questions,
                interviewRound: chunk.round ?? 1,
                interviewAmbiguity: chunk.ambiguity ?? 0,
                isStreaming: false,
              })
            }
            break
          }

          case 'tool_approval': {
            if (chunk.id && chunk.tool_name) {
              set({
                activeToolApproval: {
                  id: chunk.id as string,
                  toolName: chunk.tool_name as string,
                  reason: (chunk.reason as string) || '',
                },
                isStreaming: false,
              })
            }
            break
          }

          case 'done': {
            const sid = chunk.session_id ?? null
            const vid = chunk.project_id ?? null
            const toolCalls = chunk.tool_calls ?? []
            const phase = chunk.phase
            const evaluationPassed: boolean | undefined =
              chunk.evaluation_passed === true || chunk.evaluation_passed === 'true'
                ? true
                : chunk.evaluation_passed === false || chunk.evaluation_passed === 'false'
                  ? false
                  : undefined
            const seedId = chunk.seed_id
            const durationMs = chunk.duration_ms

            set((s) => {
              const updated = [...s.messages]

              // Convert tool_calls from done chunk into ChatActivity entries
              // (same shape as loadSession's trajectoryToActivity). These are
              // used as a fallback when RFC-015 real-time events didn't arrive.
              const doneActivities: ChatActivity[] = (
                Array.isArray(toolCalls) ? toolCalls : []
              ).map((tc: ToolCallSummary, i: number) => ({
                id: `done-tc-${i}`,
                type: 'tool_call' as const,
                timestamp: new Date().toISOString(),
                toolName: tc.tool_name ?? tc.tool,
                toolCallId: `done-${i}`,
                toolArgs:
                  typeof tc.input === 'string'
                    ? (() => {
                        try {
                          return JSON.parse(tc.input)
                        } catch {
                          return undefined
                        }
                      })()
                    : undefined,
                outputSummary:
                  typeof tc.output === 'string'
                    ? tc.output
                    : JSON.stringify(tc.output ?? '', null, 2),
                durationMs: tc.duration_ms,
                isError: false,
                isRunning: false,
              }))

              // Find the last assistant message to attach metadata.
              // If none exists yet (Ouroboros mode: execution completes
              // without any prior token chunk), create one so the
              // completion metadata has a home.
              const lastAssistantIdx = [...updated]
                .reverse()
                .findIndex((m) => m.role === 'assistant')
              if (lastAssistantIdx < 0) {
                // No assistant message yet — create one.
                const placeholder: ChatMessage = {
                  id: crypto.randomUUID(),
                  role: 'assistant',
                  content: '',
                  timestamp: new Date().toISOString(),
                  activities: doneActivities.length > 0 ? doneActivities : undefined,
                  metadata: {
                    phase,
                    evaluation_passed: evaluationPassed,
                    seed_id: seedId,
                    duration_ms: durationMs,
                    tool_calls: Array.isArray(toolCalls) ? toolCalls : [],
                  },
                }
                return { messages: [...updated, placeholder], isStreaming: false }
              }

              const idx = updated.length - 1 - lastAssistantIdx
              const target = updated[idx]
              if (target) {
                // Merge done activities into existing ones, skipping
                // duplicates already added by RFC-015 real-time events.
                // Done activities are a fallback — only keep them when
                // no real-time activities with matching toolName exist.
                const existingActivities = target.activities ?? []
                const realTimeNames = new Set(
                  existingActivities
                    .filter(
                      (a) =>
                        a.type === 'tool_call' && a.toolCallId && !a.toolCallId.startsWith('done-'),
                    )
                    .map((a) => a.toolName),
                )
                const newActivities = doneActivities.filter((a) => !realTimeNames.has(a.toolName))

                updated[idx] = {
                  ...target,
                  id: target.id ?? crypto.randomUUID(),
                  activities:
                    existingActivities.length > 0 || newActivities.length > 0
                      ? [...existingActivities, ...newActivities]
                      : undefined,
                  metadata: {
                    phase,
                    evaluation_passed: evaluationPassed,
                    seed_id: seedId,
                    duration_ms: durationMs,
                    tool_calls: Array.isArray(toolCalls) ? toolCalls : [],
                  },
                }
              }

              return { messages: updated, isStreaming: false }
            })

            if (sid) {
              // Track the mode returned by the backend for this session
              const doneMode = chunk.mode
              const isSpec = doneMode === 'spec' || doneMode === 'ouroboros'
              set((s) => ({
                _lastDoneSessionId: sid,
                activeSessionId: sid,
                specModes: { ...s.specModes, [sid]: isSpec },
                // Only update effective specMode if this is the active session
                ...(s.activeSessionId === sid || !s.activeSessionId ? { specMode: isSpec } : {}),
              }))
            }
            if (vid) {
              set({ activeProjectId: vid, _lastDoneProjectId: vid })
            }
            break
          }
          case 'error': {
            set({ isStreaming: false })
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
        // WS auto-connect is handled by the ChatPage component — only
        // connect when the chat route is active.
        if (state.activeSessionId) {
          state.loadSession(state.activeSessionId)
        }
      },
    },
  ),
)
