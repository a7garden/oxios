import { Send, X } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { Textarea } from '@/components/ui/textarea'

interface ChatInputProps {
  value: string
  onChange: (value: string) => void
  onSend: () => void
  onCancel?: () => void
  disabled?: boolean
  isStreaming?: boolean
  connected?: boolean
}

export function ChatInput({
  value,
  onChange,
  onSend,
  onCancel,
  disabled,
  isStreaming,
  connected,
}: ChatInputProps) {
  const { t } = useTranslation()

  return (
    <div className="border-t p-4">
      <div className="flex items-end gap-2">
        <Textarea
          value={value}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' && !e.shiftKey) {
              e.preventDefault()
              onSend()
            }
          }}
          placeholder={
            connected
              ? t('chat.inputPlaceholder', 'Type a message...')
              : t('chat.waitingForConnection', 'Waiting for connection...')
          }
          disabled={disabled || !connected}
          className="min-h-[44px] max-h-[120px] resize-none"
          rows={1}
        />
        {isStreaming ? (
          <Button
            onClick={onCancel}
            variant="destructive"
            size="icon"
            aria-label={t('chat.cancel', 'Cancel')}
          >
            <X className="h-4 w-4" />
          </Button>
        ) : (
          <Button
            onClick={onSend}
            disabled={!value.trim() || !connected}
            size="icon"
            aria-label={t('common.sendMessage', 'Send')}
          >
            <Send className="h-4 w-4" />
          </Button>
        )}
      </div>
    </div>
  )
}
