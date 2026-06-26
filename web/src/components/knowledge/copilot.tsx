import { Bot, ExternalLink, Send } from 'lucide-react'
import { useCallback, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { Textarea } from '@/components/ui/textarea'
import { useKnowledgeCopilot } from '@/hooks/use-knowledge'
import { useKnowledgeStore } from '@/stores/knowledge'

export function Copilot() {
  const { t } = useTranslation()
  const currentFilePath = useKnowledgeStore((s) => s.currentFilePath)
  const openFile = useKnowledgeStore((s) => s.openFile)
  const copilot = useKnowledgeCopilot()
  const [question, setQuestion] = useState('')
  const [response, setResponse] = useState<{ content: string; referenced_notes: string[] } | null>(
    null,
  )

  const handleAsk = useCallback(async () => {
    if (!question.trim()) return
    try {
      const result = await copilot.mutateAsync({
        question: question.trim(),
        contextPath: currentFilePath ?? undefined,
      })
      setResponse(result)
    } catch {
      setResponse(null)
    }
  }, [question, currentFilePath, copilot])

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      handleAsk()
    }
  }

  return (
    <div className="flex flex-col h-full">
      {/* Input */}
      <div className="p-3 border-b space-y-2">
        <div className="flex items-center gap-2">
          <Bot className="h-4 w-4 text-muted-foreground" />
          <span className="text-sm font-medium">{t('knowledge.copilot')}</span>
        </div>
        <div className="flex gap-2">
          <Textarea
            value={question}
            onChange={(e) => setQuestion(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={t('knowledge.copilotPlaceholder')}
            className="flex-1 resize-none bg-background"
            rows={2}
          />
          <Button onClick={handleAsk} disabled={!question.trim() || copilot.isPending} size="icon">
            <Send className="h-4 w-4" />
          </Button>
        </div>
      </div>

      {/* Response */}
      <div className="flex-1 overflow-y-auto p-3">
        {copilot.isPending && (
          <div className="text-sm text-muted-foreground animate-pulse">
            {t('knowledge.copilotThinking')}
          </div>
        )}
        {copilot.isError && (
          <div className="text-sm text-destructive">{t('knowledge.copilotFailedResponse')}</div>
        )}
        {response && (
          <div className="space-y-3">
            <div className="text-sm whitespace-pre-wrap">{response.content}</div>
            {response.referenced_notes.length > 0 && (
              <div className="space-y-1">
                <p className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
                  {t('knowledge.referencedNotes')}
                </p>
                {response.referenced_notes.map((note) => (
                  <button
                    key={note}
                    type="button"
                    onClick={() => openFile(note)}
                    className="flex items-center gap-1.5 text-sm text-primary hover:underline"
                  >
                    <ExternalLink className="h-3 w-3" />
                    {note.replace(/\.md$/, '')}
                  </button>
                ))}
              </div>
            )}
          </div>
        )}
        {!response && !copilot.isPending && !copilot.isError && (
          <div className="text-sm text-muted-foreground">{t('knowledge.copilotPlaceholder')}</div>
        )}
      </div>
    </div>
  )
}
