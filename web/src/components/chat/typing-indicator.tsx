import { Bot } from 'lucide-react'

/**
 * Animated "thinking / typing" indicator shown while the agent is
 * processing a request (streaming tokens haven't started yet).
 */
export function TypingIndicator() {
  return (
    <div className="flex gap-3 my-1.5 animate-fade-in-up">
      <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-primary text-primary-foreground">
        <Bot className="h-4 w-4" />
      </div>
      <div className="max-w-[80%]">
        <div className="rounded-lg px-4 py-2.5 bg-muted">
          <div className="flex items-center gap-1.5">
            <span className="h-2 w-2 rounded-full bg-muted-foreground/60 animate-[typing-bounce_1.4s_ease-in-out_infinite]" />
            <span className="h-2 w-2 rounded-full bg-muted-foreground/60 animate-[typing-bounce_1.4s_ease-in-out_0.2s_infinite]" />
            <span className="h-2 w-2 rounded-full bg-muted-foreground/60 animate-[typing-bounce_1.4s_ease-in-out_0.4s_infinite]" />
          </div>
        </div>
      </div>
    </div>
  )
}
