import { useState, useRef, useEffect } from 'react'
import { Send } from 'lucide-react'
import { useChatMessages, useChatAppend } from '@/hooks/use-knowledge'
import { Button } from '@/components/ui/button'

export function KnowledgeChat() {
  const { data: messages, isLoading } = useChatMessages()
  const chatAppend = useChatAppend()
  const [input, setInput] = useState('')
  const bottomRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages])

  const handleSend = async () => {
    const text = input.trim()
    if (!text) return

    // Journal shortcut: "text jj"
    if (text.toLowerCase().endsWith(' jj')) {
      // TODO: call journal API in Phase 2
      setInput('')
      return
    }

    await chatAppend.mutateAsync(text)
    setInput('')
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      handleSend()
    }
  }

  return (
    <div className="flex flex-col flex-1 h-full">
      {/* Header */}
      <div className="px-4 py-3 border-b">
        <h2 className="text-lg font-semibold">💬 Chat</h2>
        <p className="text-sm text-muted-foreground">Free your head</p>
      </div>

      {/* Messages */}
      <div className="flex-1 overflow-y-auto p-4 space-y-3">
        {isLoading ? (
          <div className="text-center text-muted-foreground">Loading...</div>
        ) : messages && messages.length > 0 ? (
          messages.map((msg, i) => (
            <div
              key={i}
              className="rounded-lg border p-3 text-sm hover:bg-accent/30 transition-colors"
            >
              <p className="whitespace-pre-wrap">{msg}</p>
            </div>
          ))
        ) : (
          <div className="text-center text-muted-foreground py-12">
            <p className="text-2xl mb-2">🌱</p>
            <p className="font-medium">Free your head</p>
            <p className="text-sm">Drop whatever's on your mind here</p>
          </div>
        )}
        <div ref={bottomRef} />
      </div>

      {/* Input */}
      <div className="border-t p-3">
        <div className="flex gap-2">
          <textarea
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Type a message... (jj for journal)"
            className="flex-1 resize-none rounded-md border bg-background px-3 py-2 text-sm focus:outline-none focus:ring-1 focus:ring-primary"
            rows={1}
          />
          <Button
            onClick={handleSend}
            disabled={!input.trim() || chatAppend.isPending}
            size="icon"
          >
            <Send className="h-4 w-4" />
          </Button>
        </div>
      </div>
    </div>
  )
}
