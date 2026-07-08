import { create } from 'zustand'
import type { ChatActivity, ChatMessage, StreamChunk } from '@/types'
import {
  appendActivityToMessages,
  appendTokenToMessages,
  buildWsUrl,
  chunkToActivity,
  getToken,
  parseChunk,
  patchAssistantModel,
} from './chat'

// ---------------------------------------------------------------------------
// QuickAsk — one-shot, non-persisted question/answer store.
//
// Opens its own short-lived WebSocket, sends `ephemeral: true` so the backend
// skips session persistence (chat.rs ephemeral flag), streams the reply with
// the same transparency chunks as /chat, and tears down on `done`/close.
// No session is ever written to StateStore; nothing appears in the sidebar.
// See docs/designs/2026-07-05-one-shot-quick-ask-design.md.
//
// Shares the pure chunk parsers / WS URL builder with chat.ts (no duplication).
// Token batching uses the same RAF pattern as chat.ts so the UX is identical.
// ---------------------------------------------------------------------------

export interface CapturedExchange {
  prompt: string
  reply: string
  activities: ChatActivity[]
  model?: string
  sessionId?: string
}

interface QuickAskState {
  open: boolean
  messages: ChatMessage[]
  isStreaming: boolean
  pendingModel: string | null
  /** From engine config (Settings → One-shot model); falls back to default. */
  quickAskModel: string | null
  /** Captured exchange for "promote to chat" (§5.6). */
  lastExchange: CapturedExchange | null
  /** Active tool-approval request id, if the one-shot triggers one. */
  activeToolApproval: { id: string; toolName: string; reason: string } | null
  /** User messages queued while an assistant turn is streaming. Drained
   *  (in order) when the turn completes via `done`/`error`; cleared on
   *  cancel / close / reset. Mirrors chat.ts _pendingQueue. */
  _pendingQueue: string[]
  _ws: WebSocket | null

  openQuickAsk: () => void
  closeQuickAsk: () => void
  setQuickAskModel: (model: string | null) => void
  send: (content: string) => void
  /** Cancel the in-flight stream and discard the queue. */
  cancel: () => void
  resolveToolApproval: (id: string, approved: boolean) => Promise<void>
  reset: () => void
  _drainPendingQueue: () => void
}

// ---------------------------------------------------------------------------
// Token batching — same RAF pattern as chat.ts.
//
// Each incoming token chunk previously rebuilt the entire messages array
// (O(n) per token → O(n×t) for a response of t tokens across n messages),
// triggering a Zustand subscriber re-render on every token. We instead
// accumulate token content in a module-scoped buffer and flush it at most once
// per animation frame. Any non-token chunk flushes synchronously first so
// streamed text is never lost when a tool/done/error event arrives mid-stream.
// ---------------------------------------------------------------------------
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
  useQuickAskStore.setState((s) => ({
    messages: appendTokenToMessages(s.messages, content, {
      placeholderModel: s.pendingModel ?? s.quickAskModel,
    }),
  }))
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

export const useQuickAskStore = create<QuickAskState>((set, get) => ({
  open: false,
  messages: [],
  isStreaming: false,
  pendingModel: null,
  quickAskModel: null,
  lastExchange: null,
  activeToolApproval: null,
  _pendingQueue: [],
  _ws: null,

  openQuickAsk: () => {
    // Opening while streaming focuses the existing dialog (singleton).
    if (get().isStreaming) {
      set({ open: true })
      return
    }
    set({ open: true, messages: [], lastExchange: null, pendingModel: null })
  },

  closeQuickAsk: () => {
    discardPendingTokens()
    const ws = get()._ws
    if (ws && ws.readyState === WebSocket.OPEN) ws.close()
    set({ open: false, _ws: null, isStreaming: false, _pendingQueue: [] })
  },

  setQuickAskModel: (model) => set({ quickAskModel: model }),

  send: (content) => {
    const { quickAskModel, isStreaming, _ws, messages } = get()
    if (!content.trim()) return

    // Queue if currently streaming (same pattern as chat.ts _pendingQueue).
    // The caller has already cleared the textarea; we just stash the content.
    // It is dispatched (and added to the message list) when the done/error
    // handler drains the queue — so there are no ghost messages to clean up.
    if (isStreaming) {
      set((s) => ({ _pendingQueue: [...s._pendingQueue, content] }))
      return
    }

    const now = new Date().toISOString()
    const userMsg: ChatMessage = {
      id: crypto.randomUUID(),
      role: 'user',
      content,
      timestamp: now,
    }
    // Optimistic assistant placeholder for streaming tokens.
    const assistantMsg: ChatMessage = {
      id: crypto.randomUUID(),
      role: 'assistant',
      content: '',
      timestamp: now,
      activities: [],
    }
    const baseMessages = [...messages, userMsg, assistantMsg]
    set({ messages: baseMessages, isStreaming: true, pendingModel: null })

    const connectAndSend = async () => {
      try {
        const ws =
          _ws && _ws.readyState === WebSocket.OPEN ? _ws : new WebSocket(await buildWsUrl())
        set({ _ws: ws })

        const sendPayload = () => {
          ws.send(
            JSON.stringify({
              type: 'message',
              content,
              ephemeral: true,
              model: quickAskModel ?? '',
            }),
          )
        }

        if (ws.readyState === WebSocket.OPEN) {
          sendPayload()
          return
        }

        ws.onopen = sendPayload
        ws.onmessage = (ev) => {
          let raw: unknown
          try {
            raw = JSON.parse(ev.data)
          } catch {
            return
          }
          handleChunk(parseChunk(raw), set, get)
        }
        ws.onerror = () => ws.close()
        ws.onclose = () => {
          if (get()._ws !== ws) return
          set({ _ws: null })
          if (get().isStreaming) {
            flushPendingTokens()
            appendError(set, '연결이 끊겼습니다. 다시 시도해 주세요.')
            get()._drainPendingQueue()
          }
        }
      } catch (err) {
        appendError(set, err instanceof Error ? err.message : '연결할 수 없습니다.')
        get()._drainPendingQueue()
      }
    }

    void connectAndSend()
  },

  cancel: () => {
    discardPendingTokens()
    const ws = get()._ws
    if (ws && ws.readyState === WebSocket.OPEN) ws.close()
    set({ isStreaming: false, _ws: null, _pendingQueue: [] })
  },

  resolveToolApproval: async (id, approved) => {
    try {
      await fetch(`/api/chat/tool-approval/${encodeURIComponent(id)}/respond`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          ...(getToken() ? { Authorization: `Bearer ${getToken()}` } : {}),
        },
        body: JSON.stringify({ approved }),
      })
    } catch {
      // Non-blocking — the backend oneshot times out on its own.
    }
    set({ activeToolApproval: null })
  },

  reset: () => {
    discardPendingTokens()
    const ws = get()._ws
    if (ws && ws.readyState === WebSocket.OPEN) ws.close()
    set({
      messages: [],
      isStreaming: false,
      pendingModel: null,
      lastExchange: null,
      activeToolApproval: null,
      _pendingQueue: [],
      _ws: null,
    })
  },

  _drainPendingQueue: () => {
    const { _pendingQueue } = get()
    if (_pendingQueue.length === 0) return
    // Shift the head before dispatching: send's normal path runs here because
    // isStreaming was just cleared by the done/error handler.
    const next = _pendingQueue[0]
    if (next === undefined) return
    set({ _pendingQueue: _pendingQueue.slice(1) })
    get().send(next)
  },
}))

// ---------------------------------------------------------------------------
// Chunk handling — operates on the store via set/get closures.
//
// Message transforms (token append, activity merge, model patch, placeholder
// creation) route through shared pure primitives imported from chat.ts
// (appendTokenToMessages / appendActivityToMessages / patchAssistantModel) so
// this store and the chat store cannot drift apart. Token batching uses the
// same RAF pattern as chat.ts. What stays quick-ask-specific: promote-capture
// on done, and the divergent done/interview/error/tool_approval side effects.
// ---------------------------------------------------------------------------

type SetFn = (
  partial: Partial<QuickAskState> | ((s: QuickAskState) => Partial<QuickAskState>),
) => void
type GetFn = () => QuickAskState

function updateAssistant(set: SetFn, fn: (m: ChatMessage) => ChatMessage): void {
  set((s) => {
    const msgs = [...s.messages]
    for (let i = msgs.length - 1; i >= 0; i--) {
      const m = msgs[i]
      if (m && m.role === 'assistant') {
        msgs[i] = fn(m)
        break
      }
    }
    return { messages: msgs }
  })
}

function appendError(set: SetFn, message: string): void {
  set({ isStreaming: false })
  updateAssistant(set, (m) => ({
    ...m,
    content: m.content ? `${m.content}\n\n⚠️ ${message}` : `⚠️ ${message}`,
  }))
}

function handleChunk(chunk: StreamChunk, set: SetFn, get: GetFn): void {
  // Flush any buffered token content before a non-token chunk so streamed text
  // is committed to the message before a tool/done/error event reads or
  // replaces the last assistant message. (Same guard as chat.ts.)
  if (chunk.type !== 'token') {
    flushPendingTokens()
  }

  switch (chunk.type) {
    case 'model': {
      // Patch the live assistant message, or stash as pendingModel for the
      // placeholder created on first token/activity (shared logic w/ chat.ts).
      const modelId = chunk.model
      if (!modelId) break
      set((s) => {
        const r = patchAssistantModel(s.messages, modelId)
        return r.pendingModel !== undefined
          ? { pendingModel: r.pendingModel }
          : { messages: r.messages }
      })
      break
    }
    case 'token':
      if (!chunk.content) break
      _pendingTokens += chunk.content
      scheduleTokenFlush()
      break
    case 'reasoning':
    case 'tool_start':
    case 'tool_progress':
    case 'tool_end':
    case 'memory':
    case 'usage': {
      const activity = chunkToActivity(chunk)
      if (activity) {
        set((s) => ({
          messages: appendActivityToMessages(s.messages, activity, {
            placeholderModel: s.pendingModel ?? s.quickAskModel ?? undefined,
          }),
        }))
      }
      break
    }
    case 'tool_approval':
      // Backend sends `id` (chat.rs:1479), NOT tool_call_id — that field is
      // reserved for RFC-015 tool-activity chunks (types/index.ts). Reading
      // tool_call_id here always yielded undefined → random-UUID fallback →
      // resolveToolApproval 404. Mirrors chat's tool_approval handler; also
      // pauses streaming while approval is pending.
      set({
        activeToolApproval: {
          id: chunk.id || crypto.randomUUID(),
          toolName: chunk.tool_name || 'tool',
          reason: chunk.reason ?? '',
        },
        isStreaming: false,
      })
      break
    case 'interview':
      // Rare for one-shot; surface as a plain message and stop streaming.
      appendError(set, '이 질문은 추가 정보가 필요합니다. 채팅에서 다시 시도해 주세요.')
      break
    case 'error':
      appendError(set, chunk.error ?? '오류가 발생했습니다.')
      get()._drainPendingQueue()
      break
    case 'done': {
      const state = get()
      const assistant = [...state.messages].reverse().find((m) => m.role === 'assistant')
      const user = [...state.messages].reverse().find((m) => m.role === 'user')
      if (assistant && user) {
        const prompt = user.content
        const reply = assistant.content
        const activities = assistant.activities ?? []
        set({
          lastExchange: {
            prompt,
            reply,
            activities,
            model: assistant.model ?? state.pendingModel ?? state.quickAskModel ?? undefined,
            sessionId: chunk.session_id,
          },
        })
      }
      set({ isStreaming: false })
      const ws = get()._ws
      if (ws && ws.readyState === WebSocket.OPEN) ws.close()
      // Queue drain: if the user queued follow-ups while this turn streamed,
      // dispatch the next one now that the turn is idle.
      get()._drainPendingQueue()
      break
    }
    default:
      break
  }
}
