export class SseClient {
  private controller: AbortController | null = null

  async connect(
    path: string,
    onEvent: (event: string, data: unknown) => void,
    onError?: (error: Error) => void,
  ) {
    this.disconnect()
    this.controller = new AbortController()
    const token = localStorage.getItem('oxios-api-key')
    const protocol = window.location.protocol
    const url = `${protocol}//${window.location.host}${path}`

    try {
      const response = await fetch(url, {
        headers: { Authorization: `Bearer ${token}` },
        signal: this.controller.signal,
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
              onEvent(currentEvent, data)
            } catch {
              onEvent(currentEvent, line.slice(6))
            }
            currentEvent = 'message'
          }
        }
      }
    } catch (err) {
      if ((err as Error).name !== 'AbortError') {
        onError?.(err as Error)
      }
    }
  }

  disconnect() {
    this.controller?.abort()
    this.controller = null
  }
}
