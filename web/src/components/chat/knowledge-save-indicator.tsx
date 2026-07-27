import { FileText, Save, Trash2 } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import {
  useKnowledgeSaves,
  useRemoveKnowledgeSave,
  useSaveToKnowledge,
} from '@/hooks/use-knowledge-saves'
import { cn } from '@/lib/utils'
import { usePortalStore } from '@/stores/portal'

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
  const pushDocument = usePortalStore((s) => s.pushDocument)
  // Highlight when this message's saved doc is the view on top of the portal
  // stack (mirrors ArtifactCard's active-ring affordance).
  const isActive = usePortalStore((s) => {
    const top = s.stack[s.stack.length - 1]
    return top?.type === 'document' && top.path === save?.knowledge_path
  })

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
      <div className="group mt-1 flex items-center gap-1 text-2xs text-muted-foreground">
        <button
          type="button"
          className={cn(
            'flex cursor-pointer items-center gap-1 transition-colors',
            isActive ? 'text-foreground' : 'hover:text-foreground',
          )}
          onClick={() => pushDocument(save.knowledge_path)}
          title={t('chat.knowledgeOpen')}
        >
          <FileText className="h-3 w-3" />
          <span>
            {t('chat.knowledgeSaved')} · {save.knowledge_path}
          </span>
        </button>
        <button
          type="button"
          className="cursor-pointer opacity-0 transition-opacity hover:text-destructive group-hover:opacity-100"
          onClick={() => setConfirmDelete(true)}
          title={t('common.delete')}
          aria-label={t('common.delete')}
        >
          <Trash2 className="h-3 w-3" />
        </button>
      </div>
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
