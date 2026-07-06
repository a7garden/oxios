import { create } from 'zustand'
import type { ChatActivity, ChatMessage, StreamChunk } from '@/types'
import { buildWsUrl, chunkToActivity, getToken, parseChunk } from './chat'

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
  _ws: WebSocket | null

  openQuickAsk: () => void
  closeQuickAsk: () => void
  setQuickAskModel: (model: string | null) => void
  send: (content: string) => void
  resolveToolApproval: (id: string, approved: boolean) => Promise<void>
  reset: () => void
}

export const useQuickAskStore = create<QuickAskState>((set, get) => ({
  open: false,
  messages: [],
  isStreaming: false,
  pendingModel: null,
  quickAskModel: null,
  lastExchange: null,
  activeToolApproval: null,
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
    const ws = get()._ws
    if (ws && ws.readyState === WebSocket.OPEN) ws.close()
    set({ open: false, _ws: null, isStreaming: false })
  },

  setQuickAskModel: (model) => set({ quickAskModel: model }),

  send: (content) => {
    const { quickAskModel, isStreaming, _ws, messages } = get()
    if (!content.trim() || isStreaming) return

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
            appendError(set, '연결이 끊겼습니다. 다시 시도해 주세요.')
          }
        }
      } catch (err) {
        appendError(set, err instanceof Error ? err.message : '연결할 수 없습니다.')
      }
    }

    void connectAndSend()
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
    const ws = get()._ws
    if (ws && ws.readyState === WebSocket.OPEN) ws.close()
    set({
      messages: [],
      isStreaming: false,
      pendingModel: null,
      lastExchange: null,
      activeToolApproval: null,
      _ws: null,
    })
  },
}))

// ---------------------------------------------------------------------------
// Chunk handling — operates on the store via set/get closures.
// Distinct from chat.ts: single assistant placeholder, no token-batch RAF
// (one-shot is short; per-token set is cheap enough), promote-capture on done.
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

function appendActivity(set: SetFn, activity: ChatActivity): void {
  updateAssistant(set, (m) => ({
    ...m,
    activities: [...(m.activities ?? []), activity],
  }))
}

function appendError(set: SetFn, message: string): void {
  set({ isStreaming: false })
  updateAssistant(set, (m) => ({
    ...m,
    content: m.content ? `${m.content}\n\n⚠️ ${message}` : `⚠️ ${message}`,
  }))
}

function handleChunk(chunk: StreamChunk, set: SetFn, get: GetFn): void {
  switch (chunk.type) {
    case 'model':
      set({ pendingModel: chunk.model || null })
      break
    case 'token':
      updateAssistant(set, (m) => ({ ...m, content: m.content + (chunk.content ?? '') }))
      break
    case 'reasoning':
    case 'tool_start':
    case 'tool_progress':
    case 'tool_end':
    case 'memory':
    case 'usage':
    case 'phase': {
      const activity = chunkToActivity(chunk)
      if (activity) appendActivity(set, activity)
      break
    }
    case 'tool_approval':
      set({
        activeToolApproval: {
          id: chunk.tool_call_id || crypto.randomUUID(),
          toolName: chunk.tool_name || 'tool',
          reason: chunk.reason ?? '',
        },
      })
      break
    case 'interview':
      // Rare for one-shot; surface as a plain message and stop streaming.
      appendError(set, '이 질문은 추가 정보가 필요합니다. 채팅에서 다시 시도해 주세요.')
      break
    case 'error':
      appendError(set, chunk.error ?? '오류가 발생했습니다.')
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
            model: state.pendingModel ?? state.quickAskModel ?? undefined,
            sessionId: chunk.session_id,
          },
        })
      }
      set({ isStreaming: false })
      const ws = get()._ws
      if (ws && ws.readyState === WebSocket.OPEN) ws.close()
      break
    }
    default:
      break
  }
}
