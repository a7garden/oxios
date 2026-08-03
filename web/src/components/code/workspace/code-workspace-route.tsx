import { useEffect } from 'react'
import { useNavigate, useParams } from '@tanstack/react-router'
import { useCodeSessionStore } from '@/stores/code/code-session'
import { codeApi } from '@/lib/code-api'
import { CodeWorkspace } from './code-workspace'

/**
 * Route component for /code/$sessionId.
 * Fetches session data on mount, renders the full IDE workspace.
 */
export function CodeWorkspaceRoute() {
  const { sessionId } = useParams({ from: '/code/$sessionId' })
  const navigate = useNavigate()
  const { setSession, setSessionState, reset } = useCodeSessionStore()

  useEffect(() => {
    let cancelled = false

    async function loadSession() {
      try {
        const data = await codeApi.getSession(sessionId)
        if (cancelled) return
        setSession(data.session)
        setSessionState({
          pending_changes: data.pending_changes,
          checkpoints: data.checkpoints,
          git_branch: data.git_branch,
        })
      } catch {
        if (!cancelled) navigate({ to: '/code' })
      }
    }

    loadSession()
    return () => {
      cancelled = true
      reset()
    }
  }, [sessionId, setSession, setSessionState, navigate, reset])

  return <CodeWorkspace />
}
