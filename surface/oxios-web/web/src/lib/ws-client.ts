export type WsMessageHandler = (data: unknown) => void

export class WsClient {
  private ws: WebSocket | null = null
  private url: string
  private token: string
  private onMessage: WsMessageHandler
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null
  private reconnectAttempts = 0
  private maxReconnectAttempts = 10
  /** Queue for messages sent before the socket is open. */
  private pendingQueue: string[] = []
  private _disposed = false

  constructor(path: string, token: string, onMessage: WsMessageHandler) {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
    this.url = `${protocol}//${window.location.host}${path}`
    this.token = token
    this.onMessage = onMessage
  }

  connect() {
    if (this._disposed) return
    const separator = this.url.includes('?') ? '&' : '?'
    this.ws = new WebSocket(`${this.url}${separator}token=${this.token}`)

    this.ws.onopen = () => {
      this.reconnectAttempts = 0
      // Flush any messages queued while the socket was connecting.
      while (this.pendingQueue.length > 0) {
        const msg = this.pendingQueue.shift()!
        this.ws!.send(msg)
      }
    }

    this.ws.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data as string)
        this.onMessage(data)
      } catch {
        this.onMessage(event.data)
      }
    }

    this.ws.onclose = () => {
      this.scheduleReconnect()
    }

    this.ws.onerror = () => {
      this.ws?.close()
    }
  }

  send(data: unknown) {
    const serialized = JSON.stringify(data)
    if (this.ws?.readyState === WebSocket.OPEN) {
      this.ws.send(serialized)
    } else {
      // Socket not open yet — queue for delivery on connect.
      this.pendingQueue.push(serialized)
    }
  }

  close() {
    this._disposed = true
    this.pendingQueue = []
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer)
      this.reconnectTimer = null
    }
    if (this.ws) {
      // Detach handlers to prevent close() from triggering reconnect.
      const ws = this.ws
      this.ws = null
      ws.onclose = null
      ws.onerror = null
      ws.close()
    }
  }

  private scheduleReconnect() {
    if (this._disposed) return
    if (this.reconnectAttempts >= this.maxReconnectAttempts) return
    const delay = Math.min(1000 * 2 ** this.reconnectAttempts, 30000)
    this.reconnectTimer = setTimeout(() => {
      this.reconnectAttempts++
      this.connect()
    }, delay)
  }
}
