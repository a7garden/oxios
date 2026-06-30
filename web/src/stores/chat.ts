import { create } from 'zustand'
import { persist } from 'zustand/middleware'
import type {
  ChatActivity,
  ChatMessage,
  InterviewAnswer,
  InterviewQuestion,
  Project,
  StreamChunk,
  ToolCallContext,
  ToolCallSummary,
} from '@/types'
import { useAuthStore } from './auth'

// ---------------------------------------------------------------------------
// Persisted state (survives tab switches)
// ---------------------------------------------------------------------------

interface PersistedState {
  /** Last active session ID (null = no conversation started yet). */
  activeSessionId: string | null
  /** Project ID associated with the active session (grouping). */
  activeProjectId: string | null
  /** RFC-025: Active Mount IDs (comma-separated, primary first). */
  activeMountIds: string | null
  /** RFC-032: Active role hint (null = no role; uses default model). */
  activeRole: string | null
  /** Model override id (null = follow default / role). Persisted across reloads. */
  activeModelId: string | null
}

const PERSIST_KEY = 'oxios-chat-persist'

// ---------------------------------------------------------------------------
// Runtime state
// ---------------------------------------------------------------------------

interface ChatRuntimeState {
  /** All messages in the current session (restored from /api/sessions/:id). */
  messages: ChatMessage[]
  isStreaming: boolean
  /** Buffer for the backend model-announcement chunk (`type: 'model'`) that
   *  arrives before the first token; consumed when the assistant placeholder
   *  is created. Null when no turn is in flight. */
  pendingModel: string | null
  /** WebSocket connection state. */
  connected: boolean
  /** Queue of messages waiting for WS connection. */
  _sendQueue: string[]
  /** The session ID from the last "done" chunk. */
  _lastDoneSessionId: string | null
  /** The project ID from the last "done" chunk. */
  _lastDoneProjectId: string | null
  /** AI-detected project (Phase 2 stub, always null). */
  detectedProject: Project | null
  /** RFC-025: detected mount tag from the last orchestrator response. */
  detectedMountTag: string | null
  /** RFC-025: detected mount IDs from the last orchestrator response. */
  detectedMountIds: string[]
  /** IDs of dismissed detection badges. */
  dismissedProjectIds: string[]
  /** Active structured interview questions (null = no interview active). */
  activeInterview: InterviewQuestion[] | null
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

  // ── WebSocket lifecycle (encapsulated, not persisted) ──
  /** WebSocket instance managed by the store. */
  _ws: WebSocket | null
  /** Reconnect timer (exponential backoff). */
  _reconnectTimer: number | null
  /** Reconnect attempt counter. */
  _reconnectAttempts: number
  /** RFC-024 SP2 (B4): client-side keepalive ping timer. Fires every
   *  `WS_CLIENT_PING_MS` so the server's pong-deadline is reset even
   *  when no app-level message is flowing. */
  _pingTimer: number | null
  /** RFC-024 SP2 (C2): highest `seq` we have observed on this WS.
   *  Persisted in `sessionStorage` so a hard refresh / tab reopen can
   *  resume the stream from the next message. */
  _lastSeq: number
  /** RFC-024 SP2 (C3): ring of recently-seen `msg.id` values for
   *  dedup. The replay buffer can return the same message twice
   *  (e.g. during a fast reconnect), so we drop ids we've already
   *  applied. Capacitied at `DEDUP_RING_MAX`. */
  _seenMsgIds: string[]
}

// ---------------------------------------------------------------------------
// Chat store — single source of truth for all chat state
// ---------------------------------------------------------------------------

interface ChatActions {
  /** Start or continue a WebSocket connection. */
  connect: () => Promise<void>
  /** Close the WebSocket and reset connection state. */
  disconnect: () => void
  /** RFC-024 SP2 (B4): cancel the client-side keepalive interval. */
  stopPingTimer: () => void
  /** Send a message using the active session. */
  sendMessage: (content: string) => void
  /** Load a previous session's message history from the API. */
  loadSession: (sessionId: string) => Promise<void>
  /** Start a fresh session (clears messages). */
  newSession: () => void
  /** Set the active project explicitly. */
  setActiveProject: (projectId: string | null) => void
  /** RFC-032: Set the active role hint. */
  setActiveRole: (role: string | null) => void
  /** Set the per-message model override id (null = no override). */
  setActiveModelId: (modelId: string | null) => void
  /** RFC-025: accept detected mount IDs into the active binding. */
  setActiveMountIds: (mountIds: string[] | null) => void
  /** RFC-025: Clear detected mount tag and IDs (e.g. on badge accept/dismiss). */
  clearDetectedMount: () => void
  setDetectedProject: (project: Project | null) => void
  /** Dismiss a detection badge (don't show again for this project). */
  dismissDetection: (projectId: string) => void
  /** Remove a single message by id. Used by the inline error retry flow (RFC-032). */
  removeMessage: (id: string) => void
  /** Clear persisted state (e.g. on logout). */
  clearPersist: () => void
  /** Submit interview answers and send them as a message. */
  submitInterviewResponse: (answers: InterviewAnswer[]) => void
  /** Resolve a pending tool approval (RFC-017). */
  resolveToolApproval: (id: string, approved: boolean) => Promise<void>
  /** Handle an incoming WS chunk. */
  handleChunk: (chunk: StreamChunk) => void
}

export type ChatStore = PersistedState & ChatRuntimeState & ChatActions

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

// F6: known StreamChunk type values. Unknown types are coerced to an error
// chunk so downstream handlers never operate on an unrecognised shape (which
// could produce undefined activity IDs and React key collisions).
const KNOWN_CHUNK_TYPES = new Set<StreamChunk['type']>([
  'token',
  'tool_call',
  'tool_result',
  'done',
  'error',
  'phase',
  'tool_start',
  'tool_end',
  'tool_progress',
  'memory',
  'reasoning',
  'usage',
  'interview',
  'tool_approval',
  'model',
])

function parseChunk(raw: unknown): StreamChunk {
  if (typeof raw === 'object' && raw !== null && !Array.isArray(raw)) {
    const obj = raw as Record<string, unknown>
    const t = obj.type
    if (typeof t === 'string' && KNOWN_CHUNK_TYPES.has(t as StreamChunk['type'])) {
      return obj as unknown as StreamChunk
    }
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
  context?: ToolCallContext
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

function reasoningToActivity(
  record: {
    content: string
    source: string
    timestamp: string
  },
  turnIndex: number,
): ChatActivity {
  // P4 (§7 persistence): one reasoning record per turn, restored to the
  // matching agent message's activities so the ThinkingPanel can render
  // it above the answer. `id` derives from the turn index so the existing
  // activity-card reasoning rendering paths work unchanged.
  return {
    id: `reasoning-restored-${turnIndex}`,
    type: 'reasoning',
    timestamp: record.timestamp,
    content: record.content,
    reasoningSource: record.source,
  }
}

function getToken(): string {
  return useAuthStore.getState().token || ''
}

/**
 * Whether the backend has authentication enabled.
 *
 * Learned from `GET /api/status`, which is reachable without a token exactly
 * when auth is disabled (`require_auth` skips when `auth_enabled=false`). The
 * result is cached: `auth_enabled` only changes across a daemon restart, so a
 * single probe per page session is sufficient.
 */
let authEnabledCached: boolean | null = null

async function isAuthEnabled(): Promise<boolean> {
  if (authEnabledCached !== null) return authEnabledCached
  try {
    const res = await fetch('/api/status', { headers: { Accept: 'application/json' } })
    // 401/403 means the endpoint demands auth → auth is on.
    if (res.status === 401 || res.status === 403) {
      authEnabledCached = true
      return true
    }
    if (res.ok) {
      const data = (await res.json().catch(() => null)) as { auth_enabled?: boolean } | null
      authEnabledCached = data?.auth_enabled === true
      return authEnabledCached
    }
    // 503 = subsystems still warming up (the readiness gate returns 503 until
    // the engine/state-store reach Ready/Degraded). That is not an auth signal,
    // so default to auth-off (the common case) and leave the cache unset so the
    // next connect attempt re-probes once the server is ready.
    if (res.status === 503) return false
  } catch {
    // Network error — fall through to the conservative default below.
  }
  // Could not determine: default to auth-enabled so a protected deployment is
  // never silently bypassed. The common auth-off case resolves via the 200
  // path above; this only governs genuine request failures.
  return true
}

async function buildWsUrl(): Promise<string> {
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
  const base = `${protocol}//${window.location.host}/api/chat/stream`

  // When auth is disabled (the default for local single-user deployments),
  // connect without credentials. The backend skips ticket/token validation,
  // and a browser WebSocket cannot carry a Bearer header anyway — so blocking
  // the connection on a missing token only stranded deployments that have no
  // login UI to set one.
  if (!(await isAuthEnabled())) {
    return base
  }

  const token = getToken()

  // Auth is enabled — a token is mandatory.
  if (!token) {
    throw new Error('Cannot open WebSocket: not authenticated')
  }

  // Prefer a short-lived ticket so the token itself never appears in the URL
  // (URLs are logged by proxies and may leak via Referer).
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
        return `${base}?ticket=${encodeURIComponent(data.ticket)}`
      }
    }
  } catch {
    // Ticket endpoint not available — fall through to token query param.
  }

  // F3: WS cannot use custom headers, so the token must travel as a query
  // parameter. This is strictly better than the previous behaviour which sent
  // a completely unauthenticated request when the ticket endpoint failed.
  return `${base}?token=${encodeURIComponent(token)}`
}

/** Max reconnect attempts before giving up. */
const MAX_RECONNECT_ATTEMPTS = 5

// RFC-024 SP2 (B4): client-side keepalive interval. Independent of the
// server's 20 s ping — sending our own ping every 25 s means the
// server's 60 s pong-deadline is reset on either side's traffic, so
// the connection survives NAT/proxy timeouts that fire anywhere from
// 30 s (aggressive) to 5 min (lenient).
const WS_CLIENT_PING_MS = 25_000

// RFC-024 SP2 (C3): cap on the dedup ring. 256 is generous — the
// server's replay buffer is 512 by default, so anything we've seen
// in the last 256 messages we will not apply twice.
const DEDUP_RING_MAX = 256

// RFC-024 SP2 (C2): sessionStorage keys. We deliberately use
// sessionStorage (not localStorage): the cursor is per-tab, and a
// different tab's session/project should not contaminate this one.
const SS_LAST_SEQ_KEY = 'oxios:ws:last_seq'
const SS_SEEN_IDS_KEY = 'oxios:ws:seen_ids'

function loadLastSeq(): number {
  try {
    const raw = sessionStorage.getItem(SS_LAST_SEQ_KEY)
    if (!raw) return 0
    const n = Number.parseInt(raw, 10)
    return Number.isFinite(n) && n >= 0 ? n : 0
  } catch {
    return 0
  }
}

function saveLastSeq(seq: number): void {
  try {
    sessionStorage.setItem(SS_LAST_SEQ_KEY, String(seq))
  } catch {
    // sessionStorage may be unavailable (private mode, quota); degrade silently.
  }
}

function loadSeenIds(): string[] {
  try {
    const raw = sessionStorage.getItem(SS_SEEN_IDS_KEY)
    if (!raw) return []
    const parsed = JSON.parse(raw)
    return Array.isArray(parsed) ? parsed.filter((v): v is string => typeof v === 'string') : []
  } catch {
    return []
  }
}

function saveSeenIds(ids: string[]): void {
  try {
    sessionStorage.setItem(SS_SEEN_IDS_KEY, JSON.stringify(ids))
  } catch {
    // see saveLastSeq
  }
}

// Returns true if this is a new id (caller should apply), false if
// we have seen it before (caller should drop). Mutates `ring` in place.
function markSeen(id: string, ring: string[]): boolean {
  if (ring.includes(id)) return false
  ring.push(id)
  // Cap the ring so a burst of replayed messages does not leave the
  // dedup set carrying entries the user will never see again. We
  // drop down to DEDUP_RING_MAX (rather than splice one at a time)
  // so a flood of replays after a long offline period is O(1) total.
  if (ring.length > DEDUP_RING_MAX) ring.length = DEDUP_RING_MAX
  return true
}
// F9: Token-streaming batching
// ---------------------------------------------------------------------------
// Each incoming token chunk previously rebuilt the entire messages array
// (O(n) per token → O(n×t) for a response of t tokens across n messages),
// triggering a Zustand subscriber re-render on every token. We instead
// accumulate token content in a module-scoped buffer and flush it at most once
// per animation frame. Any non-token chunk flushes synchronously first so
// streamed text is never lost when a tool/done/error event arrives mid-stream.
let _pendingTokens = ''
let _tokenRafId: number | null = null

function flushPendingTokens(): void {
  if (_tokenRafId !== null) {
    cancelAnimationFrame(_tokenRafId)
    _tokenRafId = null
  }
  if (!_pendingTokens) return
  const content = _pendingTokens
  _pendingTokens = ''
  useChatStore.setState((s) => {
    const msgs = s.messages
    const last = msgs[msgs.length - 1]
    if (last?.role === 'assistant') {
      // Only the last element changes — copy once and replace in place.
      const next = msgs.slice()
      next[next.length - 1] = { ...last, content: last.content + content }
      return { messages: next }
    }
    return {
      messages: [
        ...msgs,
        {
          id: crypto.randomUUID(),
          role: 'assistant' as const,
          content,
          timestamp: new Date().toISOString(),
        },
      ],
    }
  })
}

function scheduleTokenFlush(): void {
  if (_tokenRafId !== null) return
  _tokenRafId = requestAnimationFrame(() => {
    _tokenRafId = null
    flushPendingTokens()
  })
}

function discardPendingTokens(): void {
  if (_tokenRafId !== null) {
    cancelAnimationFrame(_tokenRafId)
    _tokenRafId = null
  }
  _pendingTokens = ''
}

// ---------------------------------------------------------------------------
// Store definition
// ---------------------------------------------------------------------------

export const useChatStore = create<ChatStore>()(
  persist(
    (set, get) => ({
      activeSessionId: null,
      activeProjectId: null,
      activeMountIds: null,
      activeRole: null,
      activeModelId: null,

      // ── Runtime ──
      messages: [],
      isStreaming: false,
      pendingModel: null as string | null,
      connected: false,
      _sendQueue: [],
      _lastDoneSessionId: null,
      _lastDoneProjectId: null,
      detectedProject: null,
      detectedMountTag: null,
      detectedMountIds: [],
      dismissedProjectIds: [],
      activeInterview: null,
      interviewRound: 0,
      interviewAmbiguity: 0,
      activeToolApproval: null,
      // WebSocket lifecycle
      _ws: null,
      _reconnectTimer: null,
      _reconnectAttempts: 0,
      _pingTimer: null,
      // RFC-024 SP2 (C2): restore the seq cursor from sessionStorage
      // so a hard refresh / new tab can resume the stream without
      // gaps. Both values are best-effort — if the server's buffer
      // is older than the saved cursor, the server emits a `resync`
      // chunk and the client falls back to a full state refresh.
      _lastSeq: loadLastSeq(),
      _seenMsgIds: loadSeenIds(),
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

        let url: string
        try {
          url = await buildWsUrl()
        } catch {
          // F3: not authenticated — abort the connection attempt gracefully
          // instead of letting the rejection propagate unhandled.
          return
        }
        const ws = new WebSocket(url)

        // Store reference so stale-checks work.
        set({ _ws: ws, connected: false, isStreaming: false })

        ws.onopen = () => {
          // If another connect() replaced this ws, ignore.
          if (get()._ws !== ws) return
          set({ connected: true, _reconnectAttempts: 0 })

          // RFC-024 SP2 (C2): if we have a saved cursor, ask the server
          // to replay any messages we missed while disconnected. The
          // server either broadcasts the gapless slice or, if the
          // cursor is older than its replay buffer, sends a synthetic
          // `resync` chunk so we can pull fresh state via HTTP.
          const lastSeq = get()._lastSeq
          if (lastSeq > 0) {
            ws.send(JSON.stringify({ type: 'resume', last_seq: lastSeq }))
          }

          // RFC-024 SP2 (B4): start the client-side keepalive. We send
          // our own ping every WS_CLIENT_PING_MS so the server's
          // 60 s pong-deadline is reset by either side's traffic.
          // Browsers do not auto-pong application-level pings.
          get().stopPingTimer()
          const pingTimer = window.setInterval(() => {
            if (get()._ws === ws && ws.readyState === WebSocket.OPEN) {
              try {
                ws.send(JSON.stringify({ type: 'ping' }))
              } catch {
                // Send can throw if the socket was just closed between
                // the readyState check and the send; the close handler
                // will deal with the reconnect.
              }
            }
          }, WS_CLIENT_PING_MS)
          set({ _pingTimer: pingTimer })

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
            const raw = JSON.parse(event.data as string) as Record<string, unknown>
            // RFC-024 SP2 (C2): track the highest seq we have observed
            // so the next reconnect can resume from here. Persisted
            // eagerly so a crash mid-stream still leaves a usable
            // cursor.
            const seq = raw.seq
            if (typeof seq === 'number' && seq > get()._lastSeq) {
              set({ _lastSeq: seq })
              saveLastSeq(seq)
            }
            // RFC-024 SP2 (C3): drop replays of messages we have
            // already applied. The server's replay path can deliver
            // duplicates when the cursor is just inside the buffer
            // window; without dedup the user would see the same
            // token stream rendered twice.
            const msgId = raw.id
            if (typeof msgId === 'string') {
              const ring = get()._seenMsgIds
              if (!markSeen(msgId, ring)) return
              saveSeenIds(ring)
            }
            const chunk = parseChunk(raw)
            get().handleChunk(chunk)
          } catch {
            // Ignore malformed JSON
          }
        }

        ws.onclose = () => {
          // Another connect() already replaced this ws — do nothing.
          if (get()._ws !== ws) return

          // RFC-024 SP2 (B4): stop the client-side keepalive so the
          // orphaned timer does not keep firing after the socket is
          // gone (it would silently fail in `onopen` above, but the
          // interval itself would survive until disconnect() or a new
          // connect()).
          get().stopPingTimer()

          set({ connected: false, isStreaming: false, _ws: null })

          // Auto-reconnect with exponential backoff.
          const attempt = get()._reconnectAttempts
          if (attempt >= MAX_RECONNECT_ATTEMPTS) return

          const delay = 1000 * 2 ** attempt
          window.setTimeout(() => {
            set({ _reconnectTimer: null })
            // Only reconnect if no new connection was established in the meantime.
            if (get()._ws === null) {
              set({ _reconnectAttempts: attempt + 1 })
              get().connect()
            }
          }, delay)
        }

        ws.onerror = () => {
          if (get()._ws !== ws) return
          ws.close()
        }
      },

      // RFC-024 SP2 (B4): cancel the keepalive interval. Safe to
      // call from `disconnect`, the close handler, or the start of
      // a new `connect()` to avoid overlapping timers.
      stopPingTimer() {
        const t = get()._pingTimer
        if (t !== null) {
          window.clearInterval(t)
          if (get()._pingTimer === t) set({ _pingTimer: null })
        }
      },

      disconnect() {
        const { _ws, _reconnectTimer } = get()

        // Stop any pending reconnect.
        if (_reconnectTimer) clearTimeout(_reconnectTimer)

        // RFC-024 SP2 (B4): kill the keepalive timer.
        get().stopPingTimer()

        if (_ws) {
          // Detach handlers before closing to prevent onclose from
          // triggering auto-reconnect.
          _ws.onopen = null
          _ws.onmessage = null
          _ws.onclose = null
          _ws.onerror = null
          _ws.close()
        }

        // F9: flush any buffered tokens before tearing down the connection so
        // the final streamed content is committed to the message.
        flushPendingTokens()
        set({
          connected: false,
          isStreaming: false,
          _ws: null,
          _reconnectTimer: null,
          _reconnectAttempts: 0,
        })
      },

      sendMessage(content: string) {
        const {
          activeSessionId,
          activeProjectId,
          activeMountIds,
          activeRole,
          activeModelId,
          connected,
          connect,
          _ws,
        } = get()

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

        // Send via WebSocket with session context.
        // The backend WS handler reads `model` and writes it into
        // `model_override` metadata, which the orchestrator honours
        // at priority 1 (above role routing and default).
        const payload: Record<string, unknown> = {
          type: 'message',
          content,
          session_id: activeSessionId ?? '',
          // Web-C2: backend WS handler reads singular `project_id`
          project_id: activeProjectId ?? '',
          mount_ids: activeMountIds ?? '',
          // RFC-032: role hint for model routing
          role: activeRole ?? '',
          // Per-message model override (or last-picked persistent one).
          model: activeModelId ?? '',
        }
        _ws.send(JSON.stringify(payload))
      },

      async loadSession(sessionId: string) {
        if (!sessionId) return
        // F9: discard buffered tokens from any prior streaming session.
        discardPendingTokens()
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
            context?: ToolCallContext
          }> = data.trajectory_steps ?? []
          const trajectoryActivities = trajectorySteps.map(trajectoryToActivity)
          const reasoningRecords: Array<{
            content: string
            source: string
            timestamp: string
          }> = data.reasoning_records ?? []

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
              const range = agentMsg.trajectory_range
              let activitiesForThisTurn: ChatActivity[] | undefined
              if (range && trajectoryActivities.length > 0) {
                activitiesForThisTurn = trajectoryActivities.slice(range.start, range.end)
                if (activitiesForThisTurn.length === 0) activitiesForThisTurn = undefined
              } else {
                const isLast = i === maxLen - 1
                if (isLast && trajectoryActivities.length > 0) {
                  activitiesForThisTurn = trajectoryActivities
                }
              }
              // P4 (§7 persistence): restore reasoning record for this turn.
              const reasoning = reasoningRecords[i]
              if (reasoning && reasoning.content) {
                const r = reasoningToActivity(reasoning, i)
                activitiesForThisTurn = activitiesForThisTurn ? [...activitiesForThisTurn, r] : [r]
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

          const projectId =
            data.project_id ?? data.metadata?.project_id ?? data.metadata?.project_ids ?? null

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
        // F9: discard any buffered tokens from the previous session so they
        // don't leak into the new session via a late rAF callback.
        discardPendingTokens()
        set(() => ({
          messages: [],
          isStreaming: false,
          pendingModel: null,
          activeSessionId: null,
          _lastDoneSessionId: null,
          _lastDoneProjectId: null,
          activeInterview: null,
          interviewRound: 0,
          interviewAmbiguity: 0,
        }))
      },

      setActiveProject(projectId: string | null) {
        // F9: discard buffered tokens when switching projects (clears messages).
        discardPendingTokens()
        set({
          activeProjectId: projectId,
          activeSessionId: null,
          messages: [],
          detectedProject: null,
        })
      },

      setActiveMountIds(mountIds: string[] | null) {
        set({
          activeMountIds: mountIds ? mountIds.join(',') : null,
        })
      },

      setActiveRole(role: string | null) {
        set({ activeRole: role })
      },

      setActiveModelId(modelId: string | null) {
        set({ activeModelId: modelId })
      },

      removeMessage(id: string) {
        // F9: discard any buffered tokens so they don't leak into the next
        // streaming turn when the user retries an errored message.
        discardPendingTokens()
        set((s) => {
          const target = s.messages.find((m) => m.id === id)
          const wasStreaming = target?.role === 'assistant' && s.isStreaming
          return {
            messages: s.messages.filter((m) => m.id !== id),
            // If we just removed the streaming assistant placeholder, drop
            // isStreaming so the input is re-enabled and the user can retry.
            isStreaming: wasStreaming ? false : s.isStreaming,
          }
        })
      },

      setDetectedMountTag(tag: string | null) {
        set({ detectedMountTag: tag })
      },

      clearDetectedMount() {
        set({ detectedMountTag: null, detectedMountIds: [] })
      },

      setDetectedProject(project: Project | null) {
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
          activeMountIds: null,
          activeRole: null,
          activeModelId: null,
          messages: [],
          activeInterview: null,
          interviewRound: 0,
          interviewAmbiguity: 0,
          activeToolApproval: null,
          detectedMountTag: null,
          detectedMountIds: [],
        })
      },

      submitInterviewResponse(answers: InterviewAnswer[]) {
        const {
          _ws,
          activeInterview,
          activeSessionId,
          activeProjectId,
          activeRole,
          activeModelId,
          interviewRound,
        } = get()
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
              role: activeRole ?? '',
              // Per-message model override (or last-picked persistent one).
              model: activeModelId ?? '',
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

      handleChunk(chunk) {
        // F9: flush any buffered token content before a non-token chunk so
        // streamed text is committed to the message before a tool/done/error
        // event reads or replaces the last assistant message.
        if (chunk.type !== 'token') {
          flushPendingTokens()
        }
        switch (chunk.type) {
          // RFC-015 model mark — arrives before the first token, so buffer it
          // in `pendingModel` when no assistant message exists yet; the first
          // placeholder consumes it. Patch the live message if it already exists.
          case 'model': {
            const modelId = chunk.model
            if (!modelId) break
            const msgs = get().messages
            let idx = -1
            for (let i = msgs.length - 1; i >= 0; i--) {
              if (msgs[i]?.role === 'assistant') {
                idx = i
                break
              }
            }
            if (idx >= 0) {
              set((s) => {
                const updated = [...s.messages]
                const target = updated[idx]
                if (!target) return {}
                updated[idx] = { ...target, model: modelId }
                return { messages: updated }
              })
            } else {
              set({ pendingModel: modelId })
            }
            break
          }
          case 'token': {
            // F9: batch tokens into a single rAF flush instead of rebuilding
            // the messages array on every token.
            if (!chunk.content) break
            _pendingTokens += chunk.content
            scheduleTokenFlush()
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
                  model: get().pendingModel ?? get().activeModelId ?? undefined,
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
            // RFC-025: extract mount_tag from metadata (gateway sets it)
            const chunkExtra = chunk as unknown as Record<string, unknown>
            const mountTag = chunkExtra.mount_tag as string | undefined
            const mountIdsRaw = chunkExtra.mount_ids as string | string[] | undefined
            // Web-M4: gateway serializes mount_ids as a JSON-array string
            // (e.g. `["id1","id2"]`); splitting on comma produces garbage.
            const mountIds = Array.isArray(mountIdsRaw)
              ? mountIdsRaw
              : typeof mountIdsRaw === 'string' && mountIdsRaw.trim().startsWith('[')
                ? (() => {
                    try {
                      return JSON.parse(mountIdsRaw) as string[]
                    } catch {
                      return []
                    }
                  })()
                : mountIdsRaw
                  ? mountIdsRaw.split(',').filter(Boolean)
                  : []
            const toolCalls = chunk.tool_calls ?? []
            const phase = chunk.phase
            const evaluationPassed: boolean | undefined =
              chunk.evaluation_passed === true || chunk.evaluation_passed === 'true'
                ? true
                : chunk.evaluation_passed === false || chunk.evaluation_passed === 'false'
                  ? false
                  : undefined
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
              // If none exists yet (e.g. a task that only ran tools with
              // no token stream), create one so the
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
                  model: get().pendingModel ?? get().activeModelId ?? undefined,
                  activities: doneActivities.length > 0 ? doneActivities : undefined,
                  metadata: {
                    phase,
                    evaluation_passed: evaluationPassed,
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
                    duration_ms: durationMs,
                    tool_calls: Array.isArray(toolCalls) ? toolCalls : [],
                  },
                }
              }

              return { messages: updated, isStreaming: false }
            })

            if (sid) {
              set({
                _lastDoneSessionId: sid,
                activeSessionId: sid,
              })
            }
            if (vid) {
              set({ activeProjectId: vid, _lastDoneProjectId: vid })
            }
            // RFC-025: store detected mount tag + ids for the detection badge.
            if (mountTag) {
              set({ detectedMountTag: mountTag })
            }
            if (mountIds.length > 0) {
              set({ detectedMountIds: mountIds })
            }
            break
          }
          case 'error': {
            // RFC-032: create an assistant message with the error text
            // so the user sees the failure inline rather than just a
            // loading spinner that silently stops.
            const errMsg = (chunk as unknown as Record<string, unknown>).message as
              | string
              | undefined
            // RFC-032: narrow the chunk's `kind` to the errorKind union so the
            // bubble can render kind-specific copy. Anything unrecognized
            // falls back to 'unknown' rather than an unchecked cast.
            const rawKind = (chunk as unknown as Record<string, unknown>).kind
            const errKind: 'quota_exceeded' | 'auth' | 'routing' | 'unknown' =
              rawKind === 'quota_exceeded' || rawKind === 'auth' || rawKind === 'routing'
                ? rawKind
                : 'unknown'
            const errSuggestion = (chunk as unknown as Record<string, unknown>).suggestion as
              | string
              | undefined
            const errorContent = errSuggestion
              ? `${errMsg}\n\n${errSuggestion}`
              : (errMsg ?? 'An error occurred')
            set((s) => {
              const updated = [...s.messages]
              // Add an error message after the user's last message
              const errorMsg: ChatMessage = {
                id: crypto.randomUUID(),
                role: 'assistant',
                content: errorContent,
                timestamp: new Date().toISOString(),
                model: get().pendingModel ?? get().activeModelId ?? undefined,
                metadata: {
                  isError: true,
                  errorKind: errKind,
                },
              }
              return { messages: [...updated, errorMsg], isStreaming: false }
            })
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
        activeMountIds: state.activeMountIds,
        activeRole: state.activeRole,
        activeModelId: state.activeModelId,
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
