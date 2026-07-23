import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api } from '@/lib/api-client'

/** Credential status for an integration (matches backend `CredentialStatus`). */
export interface CredentialStatus {
  configured: boolean
  source: string
}

/** Live detect status for an integration's CLI. */
export interface DetectedRow {
  installed: boolean
  version: string | null
  source: string
  path: string
}

/** One integration row from `GET /api/integrations`. */
export interface IntegrationRow {
  id: string
  label: string
  cli: string | null
  /** `none` | `secret` | `oauth` — drives the credential UI. */
  resolverKind: string
  /** `package_manager` | `cli_tool` | `credential_only` — UI grouping (S1). */
  kind: 'package_manager' | 'cli_tool' | 'credential_only'
  detected: DetectedRow | null
  credential: CredentialStatus
}

/** Fetch all integrations with live detect + credential status (RFC-041). */
export function useIntegrations() {
  return useQuery({
    queryKey: ['integrations'],
    queryFn: () => api.get<IntegrationRow[]>('/api/integrations'),
    staleTime: 30 * 1000,
    retry: 1,
  })
}

/** Set a static `Secret` value for an integration. */
export function useSetIntegrationCredential() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ id, value }: { id: string; value: string }) =>
      api.put(`/api/integrations/${id}/credential`, { value }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['integrations'] }),
  })
}

/** Remove a credential from an integration. */
export function useDeleteIntegrationCredential() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (id: string) => api.delete(`/api/integrations/${id}/credential`),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['integrations'] }),
  })
}

/** `POST /api/integrations/{id}/install` (M3) — returns a job_id immediately.
 * Output streams via SSE; use {@link useInstallJobStatus} to watch the job. */
export interface InstallJob {
  jobId: string
  integrationId: string
}

/** Provision an integration. Resolves with `{ jobId }`; the install runs in a
 * background kernel task. Subscribe to SSE `integration_install_*` events
 * keyed by `jobId` to observe progress and outcome. */
export function useInstallIntegration() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (id: string) => api.post<InstallJob>(`/api/integrations/${id}/install`),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['integrations'] }),
  })
}

/** Device-code response from `/oauth/start` (no device_code — H1). */
export interface DeviceCodeResponse {
  handle: string
  userCode: string
  verificationUrl: string
  expiresIn: number
}

/** Start a device-code flow for an OAuth integration. */
export function useOAuthStart() {
  return useMutation({
    mutationFn: (id: string) => api.post<DeviceCodeResponse>(`/api/integrations/${id}/oauth/start`),
  })
}

/** Poll a device-code flow. Returns `pending` | `success` | `expired` | `denied`. */
export function useOAuthPoll() {
  return useMutation({
    mutationFn: ({ id, handle }: { id: string; handle: string }) =>
      api.get<{ status: string }>(`/api/integrations/${id}/oauth/poll`, { handle }),
  })
}

import { useEvents } from '@/hooks/use-events'

/** Terminal status of an install job. */
export type InstallJobStatus =
  | { state: 'idle' }
  | { state: 'running'; line: string | null }
  | { state: 'completed'; command: string; output: string; exitCode: number | null }
  | { state: 'failed'; error: string }

/** Type guard: an Oxios SSE event with the discriminator + `jobId` we route by. */
function isInstallEvent<
  T extends { integration_install_started?: unknown } & Record<string, unknown>,
>(e: unknown, type: string): e is T & { type: string; jobId: string } {
  return (
    typeof e === 'object' &&
    e !== null &&
    'type' in e &&
    (e as { type: unknown }).type === type &&
    'jobId' in e &&
    typeof (e as { jobId: unknown }).jobId === 'string'
  )
}

/** Narrow an install event payload to its typed fields. Returns `null` if
 * the event shape doesn't match expectation — the caller treats that as
 * "ignore this event" rather than risking a wrong read. */
function readInstallEvent(e: unknown, type: string): Record<string, unknown> | null {
  if (!isInstallEvent<Record<string, unknown>>(e, type)) return null
  return e as Record<string, unknown>
}

/** Watch the SSE stream for events matching `jobId`. Returns the latest
 * status; transitions to a terminal state on `_completed` / `_failed`. The
 * Integrations list query is invalidated on terminal events so the badge
 * flips to "installed" without a manual refresh. */
export function useInstallJobStatus(jobId: string | null): InstallJobStatus {
  const qc = useQueryClient()
  const { events } = useEvents()
  if (!jobId) return { state: 'idle' }
  const matching = events.filter((e) => {
    if (typeof e !== 'object' || e === null || !('type' in e)) return false
    const t = (e as { type: unknown }).type
    return (
      typeof t === 'string' &&
      t.startsWith('integration_install_') &&
      'jobId' in e &&
      (e as unknown as { jobId: unknown }).jobId === jobId
    )
  })
  const last = matching[matching.length - 1]
  if (!last) return { state: 'idle' }

  const completed = readInstallEvent(last, 'integration_install_completed')
  if (completed) {
    qc.invalidateQueries({ queryKey: ['integrations'] })
    const command = typeof completed.command === 'string' ? completed.command : ''
    const output = typeof completed.output === 'string' ? completed.output : ''
    const exitCode = typeof completed.exitCode === 'number' ? completed.exitCode : null
    return { state: 'completed', command, output, exitCode }
  }

  const failed = readInstallEvent(last, 'integration_install_failed')
  if (failed) {
    qc.invalidateQueries({ queryKey: ['integrations'] })
    const error = typeof failed.error === 'string' ? failed.error : 'unknown error'
    return { state: 'failed', error }
  }

  const progress = readInstallEvent(last, 'integration_install_progress')
  if (progress) {
    const line = typeof progress.line === 'string' ? progress.line : null
    return { state: 'running', line }
  }

  // Started is the initial non-terminal — treat as running with no line yet.
  return { state: 'running', line: null }
}
