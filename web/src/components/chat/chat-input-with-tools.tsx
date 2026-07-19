// ChatInputWithTools — ChatInput + ChatInputActionBar composed
// Wraps the existing ChatInput with LobeHub-inspired action bar
// (web search, knowledge base, file upload) rendered below the textarea.

import { useState, useCallback } from 'react'
import { ChatInputActionBar } from './chat-input-action-bar'
import type { AttachedFile } from './chat-input-action-bar'
import { ChatInput } from './chat-input'
import type { ContextAttachment } from './chat-input'

interface ChatInputWithToolsProps {
  value: string
  onChange: (value: string) => void
  onSend: (content: string, contextItems: ContextAttachment[]) => void
  onCancel?: () => void
  disabled?: boolean
  isStreaming?: boolean
  connected?: boolean
  queuedCount?: number
  roles?: { name: string; model: string }[]
  activeRole?: string | null
  setActiveRole?: (role: string | null) => void
  activeModelId?: string | null
  setActiveModelId?: (id: string | null) => void
  activeMounts?: { id: string; label: string }[]
  onAttachMount?: (id: string) => void
  onRemoveMount?: (id: string) => void
  placeholder?: string
  showNewChatHint?: boolean
  enableSearchToggle?: boolean
  enableKnowledgeToggle?: boolean
  enableFileUpload?: boolean
}

export function ChatInputWithTools({
  enableSearchToggle,
  enableKnowledgeToggle,
  enableFileUpload,
  ...chatInputProps
}: ChatInputWithToolsProps) {
  const [searchEnabled, setSearchEnabled] = useState(false)
  const [knowledgeEnabled, setKnowledgeEnabled] = useState(false)
  const [attachedFiles, setAttachedFiles] = useState<AttachedFile[]>([])

  const handleToggleSearch = useCallback(() => setSearchEnabled((v) => !v), [])
  const handleToggleKnowledge = useCallback(() => setKnowledgeEnabled((v) => !v), [])

  const handleAttachFiles = useCallback((files: FileList) => {
    setAttachedFiles((prev) => [
      ...prev,
      ...Array.from(files).map((f) => ({ name: f.name, size: f.size, type: f.type })),
    ])
  }, [])

  const handleRemoveFile = useCallback((index: number) => {
    setAttachedFiles((prev) => prev.filter((_, i) => i !== index))
  }, [])

  const showActionBar = enableSearchToggle || enableKnowledgeToggle || enableFileUpload

  return (
    <div>
      <ChatInput {...chatInputProps} />
      {showActionBar && (
        <div className="w-full max-w-3xl mx-auto px-4 -mt-1">
          <ChatInputActionBar
            searchEnabled={searchEnabled}
            onToggleSearch={enableSearchToggle ? handleToggleSearch : undefined}
            knowledgeEnabled={knowledgeEnabled}
            onToggleKnowledge={enableKnowledgeToggle ? handleToggleKnowledge : undefined}
            attachedFiles={attachedFiles}
            onAttachFiles={enableFileUpload ? handleAttachFiles : undefined}
            onRemoveFile={handleRemoveFile}
          />
        </div>
      )}
    </div>
  )
}
