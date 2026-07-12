import { FileText, Save } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import {
  useKnowledgeSaves,
  useRemoveKnowledgeSave,
  useSaveToKnowledge,
} from '@/hooks/use-knowledge-saves'
import { cn } from '@/lib/utils'

interface KnowledgeSaveIndicatorProps {
  sessionId: string | null
  messageIndex: number
}

export function KnowledgeSaveIndicator({ sessionId, messageIndex }: KnowledgeSaveIndicatorProps) {
  const { t } = useTranslation()
  const [confirmDelete, setConfirmDelete] = useState(false)

  const { data: savesData } = useKnowledgeSaves(sessionId)
  const saveMutation = useSaveToKnowledge(sessionId)
  const removeMutation = useRemoveKnowledgeSave(sessionId)

  const saves = savesData?.saves ?? []
  const save = saves.find((s) => s.message_index === messageIndex)

  // Saved — show path + delete toggle
  if (save) {
    if (confirmDelete) {
      return (
        <div className="flex items-center gap-2 mt-1">
          <span className="text-2xs text-muted-foreground">{t('chat.knowledgeDeleteConfirm')}</span>
          <button
            type="button"
            className="text-2xs text-destructive hover:underline"
            onClick={() => {
              removeMutation.mutate(messageIndex)
              setConfirmDelete(false)
            }}
            disabled={removeMutation.isPending}
          >
            {t('common.delete')}
          </button>
          <button
            type="button"
            className="text-2xs text-muted-foreground hover:underline"
            onClick={() => setConfirmDelete(false)}
          >
            {t('common.cancel')}
          </button>
        </div>
      )
    }

    return (
      <button
        type="button"
        className={cn(
          'flex items-center gap-1 mt-1 text-2xs text-muted-foreground',
          'hover:text-foreground transition-colors cursor-pointer',
        )}
        onClick={() => setConfirmDelete(true)}
        title={t('chat.knowledgeClickToDelete')}
      >
        <FileText className="h-3 w-3" />
        <span>
          {t('chat.knowledgeSaved')} · {save.knowledge_path}
        </span>
      </button>
    )
  }

  // Not saved — show save button
  return (
    <button
      type="button"
      className={cn(
        'flex items-center gap-1 mt-1 text-2xs text-muted-foreground',
        'hover:text-foreground transition-colors cursor-pointer',
      )}
      onClick={() => saveMutation.mutate({ messageIndex })}
      disabled={saveMutation.isPending}
    >
      <Save className="h-3 w-3" />
      <span>{t('chat.knowledgeSave')}</span>
    </button>
  )
}
