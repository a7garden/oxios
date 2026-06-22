import { useAuthStore } from '@/stores/auth'

const MAX_RECONNECT_ATTEMPTS = 10
const BASE_DELAY_MS = 1000

export class SseClient {
  private controller: AbortController | null = null
  private reconnectAttempts = 0
  private reconnectTimer: number | null = null
  private currentPath: string | null = null
  private currentOnEvent: ((event: string, data: unknown) => void) | null = null
  private currentOnError: ((error: Error) => void) | null = null

  async connect(
    path: string,
    onEvent: (event: string, data: unknown) => void,
    onError?: (error: Error) => void,
    onOpen?: () => void,
  ) {
    this.disconnect()
    this.currentPath = path
    this.currentOnEvent = onEvent
    this.currentOnError = onError ?? null
    this.reconnectAttempts = 0
    await this.doConnect(undefined, onOpen)
  }

  private async doConnect(_unused?: undefined, onOpen?: () => void) {
    this.controller = new AbortController()
    const token = useAuthStore.getState().token
    const protocol = window.location.protocol
    const url = `${protocol}//${window.location.host}${this.currentPath}`

    try {
      const response = await fetch(url, {
        headers: {
          Authorization: `Bearer ${token}`,
          // F11: standard SSE headers — Accept prevents proxy/CDN buffering and
          // Cache-Control: no-cache avoids stale event replay.
          Accept: 'text/event-stream',
          'Cache-Control': 'no-cache',
        },
        signal: this.controller!.signal,
      })

      // F4: fetch resolves even for error responses (401/403/500). Validate
      // the status before treating the body as an event stream, otherwise the
      // client silently retries against an error page and callers believe the
      // connection succeeded (onOpen fires unconditionally).
      if (!response.ok) {
        if (response.status === 401) {
          // Token expired/invalid — stop retrying and surface the error so the
          // UI can re-authenticate.
          this.currentOnError?.(new Error(`SSE HTTP ${response.status}`))
          this.currentOnError = null
          this.currentPath = null
          return
        }
        this.currentOnError?.(new Error(`SSE HTTP ${response.status}`))
        this.scheduleReconnect()
        return
      }

      const reader = response.body?.getReader()
      if (!reader) return

      // Connection established — notify caller
      onOpen?.()

      const decoder = new TextDecoder()
      let buffer = ''

      while (true) {
        const { done, value } = await reader.read()
        if (done) break

        buffer += decoder.decode(value, { stream: true })
        const lines = buffer.split('\n')
        buffer = lines.pop() ?? ''

        let currentEvent = 'message'
        for (const line of lines) {
          if (line.startsWith('event: ')) {
            currentEvent = line.slice(7)
          } else if (line.startsWith('data: ')) {
            try {
              const data = JSON.parse(line.slice(6))
              this.currentOnEvent?.(currentEvent, data)
            } catch {
              this.currentOnEvent?.(currentEvent, line.slice(6))
            }
            currentEvent = 'message'
          }
        }
      }

      // Stream ended normally — schedule reconnect
      this.scheduleReconnect()
    } catch (err) {
      if ((err as Error).name !== 'AbortError') {
        this.currentOnError?.(err as Error)
        this.scheduleReconnect()
      }
    }
  }

  private scheduleReconnect() {
    if (this.reconnectAttempts >= MAX_RECONNECT_ATTEMPTS) return
    if (!this.currentPath) return

    const delay = BASE_DELAY_MS * 2 ** this.reconnectAttempts
    this.reconnectAttempts++

    this.reconnectTimer = window.setTimeout(() => {
      this.reconnectTimer = null
      this.doConnect()
    }, delay)
  }

  disconnect() {
    if (this.reconnectTimer !== null) {
      clearTimeout(this.reconnectTimer)
      this.reconnectTimer = null
    }
    this.reconnectAttempts = 0
    this.currentPath = null
    this.currentOnEvent = null
    this.currentOnError = null
    this.controller?.abort()
    this.controller = null
  }
}
