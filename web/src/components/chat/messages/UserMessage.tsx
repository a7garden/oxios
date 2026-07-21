// messages/UserMessage — right-aligned bubble with edit + delete.

import { Pencil, Trash2 } from 'lucide-react'
import { memo, useCallback, useState } from 'react'
import type { ChatMessage } from '@/types'
import { useChatStore } from '@/stores/chat'
import { ChatItem } from '@/components/chat/chat-item'
import type { ChatItemAvatar } from '@/components/chat/chat-item'

interface UserMessageProps {
  message: ChatMessage
}

function UserMessageImpl({ message }: UserMessageProps) {
  const { removeMessage, sendMessage } = useChatStore()
  const [editing, setEditing] = useState(false)
  const [editValue, setEditValue] = useState('')

  const startEdit = useCallback(() => {
    setEditValue(message.content)
    setEditing(true)
  }, [message.content])

  const saveEdit = useCallback(() => {
    setEditing(false)
    if (editValue.trim() && editValue !== message.content) {
      removeMessage?.(message.id)
      sendMessage(editValue)
    }
  }, [editValue, message.id, removeMessage, sendMessage])

  const handleDelete = useCallback(() => {
    removeMessage?.(message.id)
  }, [message.id, removeMessage])

  const avatar: ChatItemAvatar = { name: 'You' }

  return (
    <ChatItem
      avatar={avatar}
      placement="right"
      time={message.timestamp ? new Date(message.timestamp).getTime() : undefined}
      showTitle={false}
      actions={
        <div className="flex items-center gap-0.5">
          <button
            type="button"
            onClick={startEdit}
            title="Edit"
            className="inline-flex items-center justify-center w-7 h-7 rounded text-xs text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
          >
            <Pencil className="w-3 h-3" />
          </button>
          <button
            type="button"
            onClick={handleDelete}
            title="Delete"
            className="inline-flex items-center justify-center w-7 h-7 rounded text-xs text-muted-foreground hover:text-destructive hover:bg-muted transition-colors"
          >
            <Trash2 className="w-3 h-3" />
          </button>
        </div>
      }
    >
      {editing ? (
        <div className="flex flex-col gap-2">
          <textarea
            value={editValue}
            onChange={(e) => setEditValue(e.target.value)}
            className="w-full min-w-[300px] rounded-lg border bg-background px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-primary/30 resize-none"
            rows={Math.min(editValue.split('\n').length, 10)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault()
                saveEdit()
              }
              if (e.key === 'Escape') setEditing(false)
            }}
          />
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={saveEdit}
              className="inline-flex items-center gap-1 px-2.5 py-1 rounded-md bg-primary text-primary-foreground text-xs"
            >
              Save &amp; Resend
            </button>
            <button
              type="button"
              onClick={() => setEditing(false)}
              className="inline-flex items-center gap-1 px-2.5 py-1 rounded-md border text-xs"
            >
              Cancel
            </button>
          </div>
        </div>
      ) : (
        <div className="inline-block max-w-[85%] rounded-lg bg-muted/50 px-3 py-2 text-sm">
          {message.content}
        </div>
      )}
    </ChatItem>
  )
}

export const UserMessage = memo(UserMessageImpl)
