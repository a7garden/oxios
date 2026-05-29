const MAX_RECONNECT_ATTEMPTS = 10
const BASE_DELAY_MS = 1000

export class SseClient {
  private controller: AbortController | null = null
  private reconnectAttempts = 0
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null
  private currentPath: string | null = null
  private currentOnEvent: ((event: string, data: unknown) => void) | null = null
  private currentOnError: ((error: Error) => void) | null = null

  async connect(
    path: string,
    onEvent: (event: string, data: unknown) => void,
    onError?: (error: Error) => void,
  ) {
    this.disconnect()
    this.currentPath = path
    this.currentOnEvent = onEvent
    this.currentOnError = onError ?? null
    this.reconnectAttempts = 0
    await this.doConnect()
  }

  private async doConnect() {
    this.controller = new AbortController()
    const token = localStorage.getItem('oxios-api-key')
    const protocol = window.location.protocol
    const url = `${protocol}//${window.location.host}${this.currentPath}`

    try {
      const response = await fetch(url, {
        headers: { Authorization: `Bearer ${token}` },
        signal: this.controller!.signal,
      })

      const reader = response.body?.getReader()
      if (!reader) return

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

    const delay = BASE_DELAY_MS * Math.pow(2, this.reconnectAttempts)
    this.reconnectAttempts++

    this.reconnectTimer = setTimeout(() => {
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
