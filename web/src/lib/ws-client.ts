// Transport-agnostic WebSocket lifecycle hook (RFC-038 §10.2).
//
// Encapsulates: ticket-first URL build, exponential-backoff reconnect,
// RFC-024 SP2 B4 keepalive, stale-connection teardown. Protocol-agnostic:
// callers provide `onMessage` for any frame shape (text JSON, binary PTY
// bytes, etc.) and use `send` for either text or binary.

const MAX_RECONNECT_ATTEMPTS = 5
// RFC-024 SP2 (B4): client-side ping keeps the server's 60s pong-deadline
// alive across NAT/proxy timeouts that fire anywhere from 30s–5min.
const WS_CLIENT_PING_MS = 25_000

export type WsFrame = string | ArrayBuffer | Uint8Array

export type WsHooks = {
  onOpen?: () => void
  onMessage: (msg: WsFrame) => void
  onClose?: (code: number, reason: string) => void
  onError?: (err: unknown) => void
}

export type WsController = {
  send: (data: string | Uint8Array) => void
  close: () => void
  isOpen: () => boolean
}

function getToken(): string | null {
  try {
    return localStorage.getItem('oxios:auth:token')
  } catch {
    return null
  }
}

async function isAuthEnabled(): Promise<boolean> {
  try {
    const res = await fetch('/api/auth/status', {
      method: 'GET',
      credentials: 'same-origin',
    })
    if (!res.ok) return true
    const data = (await res.json()) as { enabled?: boolean }
    return data.enabled !== false
  } catch {
    return true
  }
}

export async function buildWsUrl(path: string): Promise<string> {
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
  const base = `${protocol}//${window.location.host}${path}`
  if (!(await isAuthEnabled())) return base
  const token = getToken()
  if (!token) throw new Error('Cannot open WebSocket: not authenticated')
  try {
    const ticketRes = await fetch(path.replace('/stream', '/ticket'), {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${token}`,
      },
    })
    if (ticketRes.ok) {
      const data = (await ticketRes.json()) as { ticket?: string }
      if (data.ticket) return `${base}?ticket=${encodeURIComponent(data.ticket)}`
    }
  } catch {
    // fall through to token
  }
  return `${base}?token=${encodeURIComponent(token)}`
}

/**
 * Connect to a WebSocket endpoint with ticket-first auth, exp-backoff
 * reconnect, and client-side keepalive. Returns a controller for sending
 * frames and a teardown that cancels reconnect + keepalive.
 */
export function connectWs(path: string, hooks: WsHooks): WsController {
  let ws: WebSocket | null = null
  let reconnectTimer: number | null = null
  let pingTimer: number | null = null
  let attempts = 0
  let isTornDown = false
  let intentionalClose = false

  function clearReconnect(): void {
    if (reconnectTimer !== null) {
      window.clearTimeout(reconnectTimer)
      reconnectTimer = null
    }
  }

  function clearPing(): void {
    if (pingTimer !== null) {
      window.clearInterval(pingTimer)
      pingTimer = null
    }
  }

  async function open(): Promise<void> {
    if (isTornDown) return
    if (ws && (ws.readyState === WebSocket.OPEN || ws.readyState === WebSocket.CONNECTING)) {
      ws.close()
    }
    clearReconnect()
    let url: string
    try {
      url = await buildWsUrl(path)
    } catch (err) {
      hooks.onError?.(err)
      return
    }
    const next = new WebSocket(url)
    ws = next
    next.onopen = () => {
      if (ws !== next) return
      attempts = 0
      startPing(next)
      hooks.onOpen?.()
    }
    next.onmessage = (ev: MessageEvent) => {
      if (ws !== next) return
      const data = ev.data
      if (typeof data === 'string') {
        hooks.onMessage(data)
      } else if (data instanceof ArrayBuffer) {
        hooks.onMessage(data)
      } else if (data instanceof Blob) {
        data.arrayBuffer().then((buf) => hooks.onMessage(buf))
      } else if (data instanceof Uint8Array) {
        const copy = new Uint8Array(data.byteLength)
        copy.set(data)
        hooks.onMessage(copy.buffer)
      }
    }
    next.onerror = (ev) => {
      hooks.onError?.(ev)
    }
    next.onclose = (ev) => {
      if (ws !== next) return
      clearPing()
      hooks.onClose?.(ev.code, ev.reason)
      if (intentionalClose || isTornDown) return
      if (attempts >= MAX_RECONNECT_ATTEMPTS) return
      const delay = 1000 * 2 ** attempts
      reconnectTimer = window.setTimeout(() => {
        reconnectTimer = null
        if (isTornDown) return
        attempts += 1
        void open()
      }, delay)
    }
  }

  function startPing(target: WebSocket): void {
    if (pingTimer !== null) return
    pingTimer = window.setInterval(() => {
      if (ws === target && target.readyState === WebSocket.OPEN) {
        try {
          target.send(JSON.stringify({ type: 'ping' }))
        } catch {
          // ignore — close handler will tear down
        }
      }
    }, WS_CLIENT_PING_MS)
  }

  void open()

  return {
    send(data: string | Uint8Array): void {
      if (!ws || ws.readyState !== WebSocket.OPEN) return
      try {
        ws.send(data as string | ArrayBuffer)
      } catch {
        // socket torn down — onclose will fire
      }
    },
    close(): void {
      intentionalClose = true
      isTornDown = true
      clearReconnect()
      clearPing()
      if (ws && (ws.readyState === WebSocket.OPEN || ws.readyState === WebSocket.CONNECTING)) {
        ws.close()
      }
      ws = null
    },
    isOpen(): boolean {
      return ws !== null && ws.readyState === WebSocket.OPEN
    },
  }
}
