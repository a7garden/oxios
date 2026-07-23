/**
 * LoginPage — token-entry screen shown when the server reports
 * `auth_enabled=true` (from `/health`) and the user has no valid
 * Bearer token in `sessionStorage`.
 *
 * Two paths to a valid session, in order:
 *
 * 1. **Zero-click loopback auto-issue** — on mount, POST
 *    `/api/auth/issue`. The server only honors this from a loopback
 *    client (RFC-042 §7.2), so the response is only delivered to a
 *    browser on the same machine as the daemon. On 200 we
 *    `setToken(response.token)` and unmount this gate. The user
 *    never sees this screen — they go straight to the dashboard.
 *
 * 2. **Manual paste** — for headless / non-loopback / failed-auto
 *    setups, the user copies the token from the terminal banner
 *    (printed at boot by `auto_issue_first_boot_token`) and pastes
 *    it. We validate via `fetch('/api/status')` with the Bearer
 *    header; 200 unmounts, 401 re-prompts.
 *
 * Valid token sources (server side, in `require_auth`):
 *   1. AuthManager hashes (`~/.oxios/api-keys.json`)
 *   2. `[engine].api_key` in config.toml
 *   3. `OXIOS_API_KEY` environment variable
 *
 * See docs/rfc-042 §7.2 for the full browser-auto-bootstrap rationale.
 */
import { KeyRound, Loader2, ShieldCheck, Wand2 } from 'lucide-react'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { useAuthStore } from '@/stores/auth'

interface IssueResponse {
  token: string
  name: string
  /** When true, the daemon's gateway binds to a non-loopback address.
   *  The browser auto-issue endpoint rejects non-loopback callers, so
   *  this flag is informational — it tells the UI it is unsafe to
   *  offer the zero-click button on a public origin. */
  loopback_only: boolean
}

export function LoginPage() {
  const { t } = useTranslation()
  const setToken = useAuthStore((s) => s.setToken)
  const [value, setValue] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [submitting, setSubmitting] = useState(false)
  /** State of the zero-click auto-issue probe. We start in
   *  `'probing'` and resolve to `'available' | 'unavailable' | 'denied'`
   *  after the mount-time request completes. `'available'` means the
   *  user can press the auto-issue button; the other two mean the
   *  form is the only path. */
  const [autoIssue, setAutoIssue] = useState<
    'probing' | 'available' | 'unavailable' | 'denied' | 'idle'
  >('probing')

  // Mount-time auto-issue probe. Runs once on first render. The server
  // rejects non-loopback callers with 403; that case is treated as
  // "the button won't help" and we go straight to the paste form.
  useEffect(() => {
    let cancelled = false
    void (async () => {
      try {
        const res = await fetch('/api/auth/issue', {
          method: 'POST',
          credentials: 'same-origin',
        })
        if (cancelled) return
        if (res.status === 200) {
          const data = (await res.json()) as IssueResponse
          setToken(data.token)
          // No need to flip autoIssue — setToken clears `unauthorized`
          // and the AppLayout unmounts this gate on the next render.
          return
        }
        if (res.status === 403) {
          // Caller is non-loopback — zero-click will never work here.
          setAutoIssue('denied')
          return
        }
        if (res.status === 503) {
          // No keys persisted yet (e.g. user suppressed the banner).
          setAutoIssue('unavailable')
          return
        }
        setAutoIssue('unavailable')
      } catch {
        if (!cancelled) setAutoIssue('unavailable')
      }
    })()
    return () => {
      cancelled = true
    }
  }, [setToken])

  const requestAutoIssue = async () => {
    setSubmitting(true)
    setError(null)
    try {
      const res = await fetch('/api/auth/issue', {
        method: 'POST',
        credentials: 'same-origin',
      })
      if (res.status === 200) {
        const data = (await res.json()) as IssueResponse
        setToken(data.token)
        return
      }
      if (res.status === 403) {
        setError(t('auth.autoIssueDenied'))
        setAutoIssue('denied')
        return
      }
      if (res.status === 503) {
        setError(t('auth.autoIssueUnavailable'))
        setAutoIssue('unavailable')
        return
      }
      setError(t('auth.serverError', { status: res.status }))
    } catch {
      setError(t('auth.networkError'))
    } finally {
      setSubmitting(false)
    }
  }

  const submit = async (e: React.FormEvent) => {
    e.preventDefault()
    const trimmed = value.trim()
    if (!trimmed) return
    setSubmitting(true)
    setError(null)
    // Optimistically store the token, then probe a cheap authenticated
    // endpoint. /api/status is ideal — it touches the kernel and is
    // always available when the daemon is up.
    setToken(trimmed)
    try {
      const res = await fetch('/api/status', {
        credentials: 'same-origin',
        headers: { Authorization: `Bearer ${trimmed}` },
      })
      if (res.ok) {
        // useAuthStore.setToken cleared unauthorized flag — AppLayout
        // will unmount the gate on the next render.
        return
      }
      if (res.status === 401) {
        setToken(null)
        setError(t('auth.invalidToken'))
        return
      }
      setError(t('auth.serverError', { status: res.status }))
    } catch {
      // Network failure — keep the token so the user can retry, but
      // surface the error. The actual API client will retry once the
      // server is reachable again.
      setError(t('auth.networkError'))
    } finally {
      setSubmitting(false)
    }
  }

  // Auto-issue probe still in flight (the request itself is one-shot
  // and we expect a sub-second response). Show a quiet placeholder so
  // the form does not flash the manual fallback unnecessarily.
  if (autoIssue === 'probing') {
    return (
      <div className="flex h-[100vh] h-dvh items-center justify-center bg-background p-4">
        <div
          className="flex items-center gap-2 text-sm text-muted-foreground"
          aria-live="polite"
          data-testid="auth-probing"
        >
          <Loader2 className="h-4 w-4 animate-spin" />
          {t('auth.connecting')}
        </div>
      </div>
    )
  }

  return (
    <div className="flex h-[100vh] h-dvh items-center justify-center bg-background p-4">
      <Card className="w-full max-w-sm">
        <CardHeader className="text-center">
          <div className="mx-auto mb-2 flex h-12 w-12 items-center justify-center rounded-full bg-primary/10 text-primary">
            <ShieldCheck className="h-6 w-6" />
          </div>
          <CardTitle className="text-xl">{t('auth.title')}</CardTitle>
          <CardDescription>{t('auth.description')}</CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          {autoIssue === 'available' && (
            <Button
              type="button"
              variant="default"
              className="w-full"
              onClick={() => void requestAutoIssue()}
              disabled={submitting}
              data-testid="auth-auto-issue"
            >
              {submitting ? (
                <>
                  <Loader2 className="h-4 w-4 animate-spin" />
                  {t('auth.connecting')}
                </>
              ) : (
                <>
                  <Wand2 className="h-4 w-4" />
                  {t('auth.autoIssueCta')}
                </>
              )}
            </Button>
          )}

          {autoIssue !== 'denied' && autoIssue !== 'available' && (
            <>
              {autoIssue === 'unavailable' && (
                <p className="text-xs text-muted-foreground">{t('auth.autoIssueUnavailable')}</p>
              )}
              <form onSubmit={submit} className="space-y-4">
                <div className="space-y-2">
                  <Label htmlFor="oxios-token">{t('auth.tokenLabel')}</Label>
                  <div className="relative">
                    <KeyRound
                      className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground"
                      aria-hidden="true"
                    />
                    <Input
                      id="oxios-token"
                      type="password"
                      autoComplete="off"
                      autoFocus
                      spellCheck={false}
                      className="pl-9 font-mono"
                      placeholder={t('auth.tokenPlaceholder')}
                      value={value}
                      onChange={(e) => setValue(e.target.value)}
                      disabled={submitting}
                      aria-invalid={error !== null}
                    />
                  </div>
                  <p className="text-xs text-muted-foreground">{t('auth.tokenHint')}</p>
                </div>
                {error && (
                  <p role="alert" className="text-sm text-destructive">
                    {error}
                  </p>
                )}
                <Button type="submit" className="w-full" disabled={submitting || !value.trim()}>
                  {submitting ? (
                    <>
                      <Loader2 className="h-4 w-4 animate-spin" />
                      {t('auth.connecting')}
                    </>
                  ) : (
                    t('auth.submit')
                  )}
                </Button>
              </form>
            </>
          )}

          {autoIssue === 'denied' && (
            <div className="space-y-2 text-center text-sm text-muted-foreground">
              <p>{t('auth.autoIssueDenied')}</p>
              <p className="text-xs">{t('auth.tokenHint')}</p>
            </div>
          )}

          {error && autoIssue === 'available' && (
            <p role="alert" className="text-sm text-destructive">
              {error}
            </p>
          )}
        </CardContent>
      </Card>
    </div>
  )
}
