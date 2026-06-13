import { FileText, Save, Trash2 } from 'lucide-react'
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

export function KnowledgeSaveIndicator({
  sessionId,
  messageIndex,
}: KnowledgeSaveIndicatorProps) {
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
          <span className="text-2xs text-muted-foreground">
            {t('chat.knowledgeDeleteConfirm', '이 노트를 삭제하시겠습니까?')}
          </span>
          <button
            className="text-2xs text-destructive hover:underline"
            onClick={() => {
              removeMutation.mutate(messageIndex)
              setConfirmDelete(false)
            }}
            disabled={removeMutation.isPending}
          >
            {t('common.delete', '삭제')}
          </button>
          <button
            className="text-2xs text-muted-foreground hover:underline"
            onClick={() => setConfirmDelete(false)}
          >
            {t('common.cancel', '취소')}
          </button>
        </div>
      )
    }

    return (
      <button
        className={cn(
          'flex items-center gap-1 mt-1 text-2xs text-muted-foreground',
          'hover:text-foreground transition-colors cursor-pointer',
        )}
        onClick={() => setConfirmDelete(true)}
        title={t('chat.knowledgeClickToDelete', '클릭하여 삭제')}
      >
        <FileText className="h-3 w-3" />
        <span>
          {t('chat.knowledgeSaved', '저장됨')} · {save.knowledge_path}
        </span>
      </button>
    )
  }

  // Not saved — show save button
  return (
    <button
      className={cn(
        'flex items-center gap-1 mt-1 text-2xs text-muted-foreground',
        'hover:text-foreground transition-colors cursor-pointer',
      )}
      onClick={() => saveMutation.mutate({ messageIndex })}
      disabled={saveMutation.isPending}
    >
      <Save className="h-3 w-3" />
      <span>{t('chat.knowledgeSave', '지식에 저장')}</span>
    </button>
  )
}
