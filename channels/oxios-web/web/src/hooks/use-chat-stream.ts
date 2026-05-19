import { useCallback, useRef, useState } from 'react'
import { WsClient } from '@/lib/ws-client'
import type { ChatMessage, StreamChunk } from '@/types'

export function useChatStream() {
  const [messages, setMessages] = useState<ChatMessage[]>([])
  const [isStreaming, setIsStreaming] = useState(false)
  const wsRef = useRef<WsClient | null>(null)

  const connect = useCallback(() => {
    const token = localStorage.getItem('oxios-api-key') || ''
    const client = new WsClient('/api/chat/stream', token, (data) => {
      const chunk = data as StreamChunk
      if (chunk.type === 'token' && chunk.content) {
        const tokenContent = chunk.content
        setMessages((prev) => {
          const updated = [...prev]
          const last = updated[updated.length - 1]
          if (last?.role === 'assistant') {
            return [...updated.slice(0, -1), { ...last, content: last.content + tokenContent }]
          }
          return [
            ...updated,
            {
              role: 'assistant' as const,
              content: tokenContent,
              timestamp: new Date().toISOString(),
            },
          ]
        })
      } else if (chunk.type === 'done') {
        setIsStreaming(false)
      } else if (chunk.type === 'error') {
        setIsStreaming(false)
      }
    })
    wsRef.current = client
    client.connect()
    return client
  }, [])

  const sendMessage = useCallback(
    (content: string) => {
      const userMsg: ChatMessage = {
        role: 'user',
        content,
        timestamp: new Date().toISOString(),
      }
      setMessages((prev) => [...prev, userMsg])
      setIsStreaming(true)

      if (!wsRef.current) connect()
      wsRef.current?.send({ type: 'message', content })
    },
    [connect],
  )

  const disconnect = useCallback(() => {
    wsRef.current?.close()
    wsRef.current = null
  }, [])

  return { messages, isStreaming, sendMessage, disconnect }
}
