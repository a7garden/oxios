// useDraftPersistence — debounced save + restore of the chat input per session.
//
// LobeHub analogue: features/ChatInput/hooks/useChatInputDraft.ts. On session
// switch the saved draft is restored; while typing the value is saved after a
// 500ms debounce; an empty value (after send) clears the draft.

import { useEffect, useRef } from 'react'
import { loadDraft, saveDraft } from '@/lib/draft-storage'

export function useDraftPersistence(
  sessionId: string | null,
  value: string,
  setValue: (v: string) => void,
): void {
  const lastSession = useRef<string | null>(null)
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null)

  // Restore the draft when the active session changes.
  useEffect(() => {
    if (sessionId !== lastSession.current) {
      lastSession.current = sessionId
      setValue(sessionId ? loadDraft(sessionId) : '')
    }
  }, [sessionId, setValue])

  // Debounced save while the value changes.
  useEffect(() => {
    if (!sessionId) return
    clearTimeout(timer.current!)
    timer.current = setTimeout(() => saveDraft(sessionId, value), 500)
    return () => clearTimeout(timer.current!)
  }, [sessionId, value])
}
