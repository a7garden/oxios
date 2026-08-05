import type { ConnectionPath, HostProfile, PhysicalRpcTransport } from './types'

export function directEndpointUrls(host: HostProfile): string[] {
  return [...new Set([host.endpoint, ...(host.endpoints?.map((e) => e.url) ?? [])])]
}

export function directPathForEndpoint(host: HostProfile, endpoint: string): ConnectionPath {
  const kind = host.endpoints?.find((e) => e.url === endpoint)?.kind
  if (kind) return kind
  try {
    const h = new URL(endpoint).hostname
    return h.endsWith('.ts.net') || /^100\.(?:\d{1,3}\.){2}\d{1,3}$/.test(h) ? 'tailscale' : 'lan'
  } catch {
    return 'lan'
  }
}

function authenticated(client: PhysicalRpcTransport, ms: number) {
  if (client.getState() === 'connected') return Promise.resolve()
  return new Promise<void>((resolve, reject) => {
    let done = false
    const off = client.onStateChange((s) => {
      if (s === 'connected') finish(resolve)
      else if (s === 'disconnected' || s === 'auth-failed') finish(() => reject(new Error(`probe ${s}`)))
    })
    const timer = setTimeout(
      () => finish(() => reject(new Error('probe authentication timed out'))),
      ms,
    )
    function finish(cb: () => void) {
      if (done) return
      done = true
      clearTimeout(timer)
      off()
      cb()
    }
  })
}

export async function openAuthenticatedDirectEndpoint(
  host: HostProfile,
  open: (url: string) => PhysicalRpcTransport,
  timeoutMs = 12_000,
): Promise<{ client: PhysicalRpcTransport; path: ConnectionPath; endpoint: string } | null> {
  const urls = directEndpointUrls(host)
  if (!urls.length) return null
  return new Promise((resolve) => {
    const clients = new Set<PhysicalRpcTransport>()
    let left = urls.length
    let settled = false
    const failed = () => {
      if (--left === 0 && !settled) {
        settled = true
        resolve(null)
      }
    }
    for (const url of urls) {
      let client: PhysicalRpcTransport
      try {
        client = open(url)
      } catch {
        failed()
        continue
      }
      clients.add(client)
      void authenticated(client, timeoutMs).then(
        () => {
          if (settled) {
            client.close()
            return
          }
          settled = true
          for (const other of clients) if (other !== client) other.close()
          resolve({ client, path: directPathForEndpoint(host, url), endpoint: url })
        },
        () => {
          client.close()
          failed()
        },
      )
    }
  })
}
