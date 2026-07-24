// ThreadView — portal view for a thread (sub-session spawned from a parent).
//
// LobeHub analogue: features/Portal/Thread/ (sub-conversation panel). Oxios
// version reuses the existing session-load + chat-render path: a thread is
// just a Session with a non-null `parent_session_id`. The view fetches
// its messages via the same /api/sessions/:id endpoint and renders them
// read-only (no inline editing — threads are independent conversations).
//
// On mount with `sessionId: null`, this view POSTs to
// /api/sessions/:parentId/threads, receives the new thread's ID, and
// patches the view in place (transition from "loading" to "ready").

import { Loader2 } from 'lucide-react'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { api } from '@/lib/api-client'
import { useChatStore } from '@/stores/chat'
import type { PortalView } from '@/stores/portal'
import { usePortalStore } from '@/stores/portal'

interface ThreadViewProps {
  view: Extract<PortalView, { type: 'thread' }>
}

interface ThreadMessage {
  content: string
  role: 'user' | 'assistant'
}

export function ThreadView({ view }: ThreadViewProps) {
  const { t } = useTranslation()
  const [creating, setCreating] = useState(view.sessionId === null)
  const [error, setError] = useState<string | null>(null)
  const [messages, setMessages] = useState<ThreadMessage[]>([])
  const [loading, setLoading] = useState(false)
  const loadSession = useChatStore((s) => s.loadSession)

  // Create the thread on mount if not yet created.
  useEffect(() => {
    if (view.sessionId !== null) return
    let cancelled = false
    void (async () => {
      try {
        const res = await api.post<{ session_id: string }>(
          `/api/sessions/${encodeURIComponent(view.parentId)}/threads`,
        )
        if (cancelled) return
        // Patch the top view in place to swap loading → ready.
        usePortalStore.getState().popView()
        usePortalStore.getState().pushView({
          type: 'thread',
          sessionId: res.session_id,
          parentId: view.parentId,
        })
      } catch (e) {
        if (!cancelled) setError(String(e))
      } finally {
        if (!cancelled) setCreating(false)
      }
    })()
    return () => {
      cancelled = true
    }
  }, [view.parentId, view.sessionId])

  // Load the thread's messages when we have a session ID.
  useEffect(() => {
    if (view.sessionId === null) return
    let cancelled = false
    setLoading(true)
    void (async () => {
      try {
        const res = await api.get<{
          user_messages: { content: string }[]
          agent_responses: { content: string }[]
        }>(`/api/sessions/${encodeURIComponent(view.sessionId ?? '')}`)
        if (cancelled) return
        const msgs: ThreadMessage[] = []
        for (let i = 0; i < res.user_messages.length; i++) {
          msgs.push({ role: 'user', content: res.user_messages[i]?.content ?? '' })
          if (res.agent_responses[i]) {
            msgs.push({
              role: 'assistant',
              content: res.agent_responses[i]?.content ?? '',
            })
          }
        }
        setMessages(msgs)
      } catch (e) {
        if (!cancelled) setError(String(e))
      } finally {
        if (!cancelled) setLoading(false)
      }
    })()
    return () => {
      cancelled = true
    }
  }, [view.sessionId])

  if (creating) {
    return (
      <div className="flex h-full items-center justify-center gap-2 text-muted-foreground">
        <Loader2 className="size-4 animate-spin" />
        <span className="text-xs">{t('portal.threadCreating')}</span>
      </div>
    )
  }

  if (error) {
    return (
      <div className="flex h-full items-center justify-center text-xs text-destructive">
        {error}
      </div>
    )
  }

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center justify-between border-b px-3 py-2 text-xs text-muted-foreground">
        <span>
          {t('portal.threadFrom')}{' '}
          <button
            type="button"
            onClick={() => loadSession(view.parentId)}
            className="font-mono text-foreground hover:underline"
          >
            {view.parentId.slice(0, 8)}
          </button>
        </span>
        <span>{messages.length} msgs</span>
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto p-3 text-xs space-y-2">
        {loading && messages.length === 0 ? (
          <div className="flex items-center gap-2 text-muted-foreground">
            <Loader2 className="size-3 animate-spin" />
            {t('portal.loading')}
          </div>
        ) : messages.length === 0 ? (
          <div className="text-muted-foreground">{t('portal.threadEmpty')}</div>
        ) : (
          messages.map((m, i) => (
            <div
              key={i}
              className={m.role === 'user' ? 'text-foreground' : 'text-muted-foreground'}
            >
              <span className="font-medium">{m.role === 'user' ? '› ' : '‹ '}</span>
              {m.content}
            </div>
          ))
        )}
      </div>
    </div>
  )
}
