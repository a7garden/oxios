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

/** Install output from `POST /api/integrations/{id}/install`. */
export interface InstallOutput {
  success: boolean
  command: string
  output: string
  exitCode: number | null
}

/** Provision an integration (runs its first applicable install spec). */
export function useInstallIntegration() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (id: string) => api.post<InstallOutput>(`/api/integrations/${id}/install`),
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
    mutationFn: (id: string) =>
      api.post<DeviceCodeResponse>(`/api/integrations/${id}/oauth/start`),
  })
}

/** Poll a device-code flow. Returns `pending` | `success` | `expired` | `denied`. */
export function useOAuthPoll() {
  return useMutation({
    mutationFn: ({ id, handle }: { id: string; handle: string }) =>
      api.get<{ status: string }>(`/api/integrations/${id}/oauth/poll`, { handle }),
  })
}
