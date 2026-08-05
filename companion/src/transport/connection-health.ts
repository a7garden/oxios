import type { ConnectionHealth, ConnectionPath, ConnectionState } from './types'

export function isTailscaleEndpoint(endpoint?: string | null): boolean {
  if (!endpoint) return false
  try {
    const host = new URL(endpoint).hostname
    return host.endsWith('.ts.net') || /^100\.(?:\d{1,3}\.){2}\d{1,3}$/.test(host)
  } catch {
    return false
  }
}

export function classifyConnection(args: {
  state: ConnectionState
  endpoint?: string | null
  path?: ConnectionPath
}): ConnectionHealth {
  const { state, endpoint, path } = args
  if (state === 'connected') return { verdict: 'connected', label: 'Connected', path }
  if (state === 'connecting' || state === 'handshaking') return { verdict: 'connecting', label: 'Connecting…', path }
  if (state === 'reconnecting') return { verdict: 'connecting', label: 'Reconnecting…', path }
  const hint = isTailscaleEndpoint(endpoint) ? 'check Tailscale' : undefined
  return {
    verdict: 'disconnected',
    label: hint ? "Can't connect — check Tailscale" : "Can't connect",
    path,
    hint
  }
}
