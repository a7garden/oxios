import { create } from 'zustand'
import { persist } from 'zustand/middleware'
import type { ChatMessage } from '@/types'
import { useAuthStore } from './auth'

// ---------------------------------------------------------------------------
// Persisted state (survives tab switches)
// ---------------------------------------------------------------------------

interface PersistedState {
  /** Last active session ID (null = no conversation started yet). */
  activeSessionId: string | null
  /** Space ID associated with the active session. */
  activeSpaceId: string | null
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
  /** The space ID from the last "done" chunk. */
  _lastDoneSpaceId: string | null
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
  /** Set the active space explicitly. */
  setActiveSpace: (spaceId: string | null) => void
  /** Clear persisted state (e.g. on logout). */
  clearPersist: () => void
  /** Handle an incoming WS chunk. */
  handleChunk: (chunk: { type: string; content?: string; error?: string; session_id?: string; space_id?: string }) => void
}

export type ChatStore = PersistedState & ChatRuntimeState & ChatActions

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
let chunkHandler: ((chunk: { type: string; content?: string; error?: string; session_id?: string; space_id?: string }) => void) | null = null

// ---------------------------------------------------------------------------
// Store definition
// ---------------------------------------------------------------------------

export const useChatStore = create<ChatStore>()(
  persist(
    (set, get) => ({
      // ── Persisted ──
      activeSessionId: null,
      activeSpaceId: null,

      // ── Runtime ──
      messages: [],
      isStreaming: false,
      connected: false,
      _sendQueue: [],
      _lastDoneSessionId: null,
      _lastDoneSpaceId: null,

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
            const chunk = JSON.parse(event.data as string)
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
        chunkHandler = (chunk) => get().handleChunk(chunk)
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
        const { activeSessionId, activeSpaceId, connected, connect } = get()

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
            space_id: activeSpaceId ?? '',
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
          const userMsgs: { content: string }[] = data.user_messages ?? []
          const agentMsgs: { content: string }[] = data.agent_responses ?? []
          const maxLen = Math.max(userMsgs.length, agentMsgs.length)
          for (let i = 0; i < maxLen; i++) {
            const userMsg = userMsgs[i]
            const agentMsg = agentMsgs[i]
            if (userMsg != null) {
              messages.push({
                role: 'user',
                content: userMsg.content,
                timestamp: data.created_at,
              })
            }
            if (agentMsg) {
              messages.push({
                role: 'assistant',
                content: agentMsg.content ?? '',
                timestamp: data.updated_at,
              })
            }
          }

          const spaceId = data.space_id ?? data.metadata?.space_id ?? null

          set({
            messages,
            activeSessionId: sessionId,
            activeSpaceId: spaceId,
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
          _lastDoneSpaceId: null,
        })
      },

      setActiveSpace(spaceId: string | null) {
        set({
          activeSpaceId: spaceId,
          activeSessionId: null,
          messages: [],
        })
      },

      clearPersist() {
        set({
          activeSessionId: null,
          activeSpaceId: null,
          messages: [],
        })
      },

      handleChunk(chunk) {
        if (chunk.type === 'token' && chunk.content) {
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
                { role: 'assistant' as const, content: chunk.content ?? '', timestamp: new Date().toISOString() },
              ],
            }
          })
        } else if (chunk.type === 'done') {
          const sid = chunk.session_id ?? null
          const vid = chunk.space_id ?? null

          if (sid) {
            set({
              isStreaming: false,
              _lastDoneSessionId: sid,
              activeSessionId: sid,
            })
          } else {
            set({ isStreaming: false })
          }

          if (vid) {
            set({ activeSpaceId: vid, _lastDoneSpaceId: vid })
          }
        } else if (chunk.type === 'error') {
          const err = chunk.error ?? 'Unknown error'
          const msgs = get().messages
          const last = msgs[msgs.length - 1]
          if (last?.role === 'assistant') {
            set({
              messages: [
                ...msgs.slice(0, -1),
                { ...last, content: last.content + `\n\n[Error: ${err}]` },
              ],
              isStreaming: false,
            })
          } else {
            set({ isStreaming: false })
          }
        }
      },
    }),
    {
      name: PERSIST_KEY,
      partialize: (state): PersistedState => ({
        activeSessionId: state.activeSessionId,
        activeSpaceId: state.activeSpaceId,
      }),
      onRehydrateStorage: () => (state) => {
        if (!state) return
        // After rehydration, if there's an active session, load its history
        if (state.activeSessionId) {
          state.loadSession(state.activeSessionId)
        }
        // Auto-connect on page load
        state.connect()
      },
    },
  ),
)