/**
 * useCodeStream — real-time coding agent streaming.
 *
 * Architecture:
 *   • POST /api/code/sessions/:id/message — triggers persona activation,
 *     agent execution, and post-turn change detection (pre-snapshot +
 *     record_write_with_original). Blocks until the agent turn completes.
 *   • WS /api/chat/stream — open purely to RECEIVE streamed chunks
 *     (token deltas, tool calls, phase updates) that the gateway
 *     broadcasts during the POST's send_and_wait. This gives the user
 *     real-time feedback without waiting for the full response.
 *
 * The gateway broadcasts every OutgoingMessage via the bridge's
 * outgoing_tx (broadcast::Sender), so intermediate chunks reach the
 * WS subscriber even though the POST handler is still blocking on
 * send_and_wait. The POST's .then() refreshes pending changes after
 * detection completes — not the WS done event — avoiding the race
 * where done fires before the POST's server-side detection finishes.
 */
import { useCallback, useEffect, useRef } from 'react'
import { codeApi } from '@/lib/code-api'
import { useCodeSessionStore } from '@/stores/code/code-session'

export function useCodeStream() {
  const wsRef = useRef<WebSocket | null>(null)
  /** ID of the assistant message currently accumulating tokens. */
  const assistantIdRef = useRef<string | null>(null)
  /** AbortController for the in-flight POST (stop button). */
  const abortRef = useRef<AbortController | null>(null)
  const sessionIdRef = useRef<string | null>(null)

  const session = useCodeSessionStore((s) => s.session)
  const addMessage = useCodeSessionStore((s) => s.addMessage)
  const appendMessageContent = useCodeSessionStore((s) => s.appendMessageContent)
  const addToolCall = useCodeSessionStore((s) => s.addToolCall)
  const setAgentRunning = useCodeSessionStore((s) => s.setAgentRunning)
  const setAgentPhase = useCodeSessionStore((s) => s.setAgentPhase)
  const setPendingChanges = useCodeSessionStore((s) => s.setPendingChanges)

  sessionIdRef.current = session?.id ?? null

  // ── WS lifecycle (receive-only) ───────────────────────────────────
  useEffect(() => {
    if (!session) return

    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
    const ws = new WebSocket(`${protocol}//${window.location.host}/api/chat/stream`)
    wsRef.current = ws

    ws.onmessage = (ev) => {
      if (typeof ev.data !== 'string') return
      let chunk: Record<string, unknown>
      try {
        chunk = JSON.parse(ev.data)
      } catch {
        return
      }

      switch (chunk.type) {
        case 'token': {
          const content = (chunk.content as string) || ''
          if (!assistantIdRef.current) {
            const id = `agent-${Date.now()}`
            assistantIdRef.current = id
            addMessage({
              id,
              role: 'assistant',
              content,
              timestamp: new Date().toISOString(),
              model: chunk.model as string | undefined,
            })
          } else {
            appendMessageContent(assistantIdRef.current, content)
          }
          break
        }
        case 'tool_start': {
          if (assistantIdRef.current) {
            addToolCall(assistantIdRef.current, {
              tool: (chunk.tool as string) ?? (chunk.name as string) ?? 'tool',
              args: (chunk.args as Record<string, unknown>) ?? {},
            })
          }
          break
        }
        case 'phase': {
          const phase = chunk.phase as string | null
          setAgentPhase(phase ? phase.charAt(0).toUpperCase() + phase.slice(1) : null)
          break
        }
        case 'reasoning': {
          const content = (chunk.content as string) || ''
          if (content) {
            if (!assistantIdRef.current) {
              const id = `agent-${Date.now()}`
              assistantIdRef.current = id
              addMessage({
                id,
                role: 'assistant',
                content: '',
                timestamp: new Date().toISOString(),
              })
            }
            appendMessageContent(assistantIdRef.current, content)
          }
          break
        }
        // 'done' and 'error' are handled by the POST lifecycle, not the
        // WS — the POST resolves after change detection completes, which
        // is the authoritative "turn finished" signal.
        default:
          break
      }
    }

    return () => {
      ws.close()
      wsRef.current = null
    }
  }, [session?.id, addMessage, appendMessageContent, addToolCall, setAgentPhase])

  // ── Send message (POST + WS receive) ──────────────────────────────
  const sendMessage = useCallback(
    (content: string, model?: string | null) => {
      if (!session) return false

      assistantIdRef.current = null
      setAgentRunning(true)
      setAgentPhase('Thinking…')

      const sid = session.id
      const ac = new AbortController()
      abortRef.current = ac

      // POST triggers the full agent pipeline: persona activation →
      // agent execution → post-turn change detection. Streaming chunks
      // arrive via the open WS during execution for real-time UX.
      fetch(`/api/code/sessions/${encodeURIComponent(sid)}/message`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ content, context_files: [], ...(model ? { model } : {}) }),
        signal: ac.signal,
      })
        .then(async (res) => {
          if (!res.ok) throw new Error(`Server responded ${res.status}`)
          // Change detection completed server-side — refresh the UI.
          try {
            const changes = await codeApi.listChanges(sid)
            setPendingChanges(changes)
          } catch {
            /* non-fatal */
          }
        })
        .catch((e) => {
          if (e instanceof Error && e.name === 'AbortError') {
            setAgentPhase('Stopped')
          } else {
            console.error('Code agent message failed:', e)
          }
        })
        .finally(() => {
          abortRef.current = null
          setAgentRunning(false)
          setAgentPhase(null)
          assistantIdRef.current = null
        })

      return true
    },
    [session, setAgentRunning, setAgentPhase, setPendingChanges],
  )

  const stop = useCallback(() => {
    abortRef.current?.abort()
    abortRef.current = null
  }, [])

  return { sendMessage, stop }
}
