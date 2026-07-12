import { FolderCheck, X } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { cn } from '@/lib/utils'
import { useChatStore } from '@/stores/chat'

/**
 * Mount 감지 배지 (RFC-025).
 *
 * "oxios" 같은 Mount 이름을 메시지에서 감지하면 표시되는 dismissible 배지.
 * sticky-primary 모델: 첫 Mount이 primary, 이후 언급은 secondary로 추가.
 */
export function MountDetectionBadge() {
  const { t } = useTranslation()
  const detectedMountTag = useChatStore((s) => s.detectedMountTag)
  const activeMountIds = useChatStore((s) => s.activeMountIds)
  const setActiveMountIds = useChatStore((s) => s.setActiveMountIds)
  const clearDetectedMount = useChatStore((s) => s.clearDetectedMount)

  if (!detectedMountTag) return null

  const handleDismiss = () => {
    // Web-m2: clear both the tag and the detected IDs.
    clearDetectedMount()
  }

  const handleAccept = () => {
    // Accept detected mounts into the active binding (sticky-primary).
    const store = useChatStore.getState()
    const detectedIds = store.detectedMountIds
    const current = activeMountIds ? activeMountIds.split(',').filter(Boolean) : []
    // Merge: keep existing primary, append new secondary mounts.
    const merged = [...new Set([...current, ...detectedIds])]
    setActiveMountIds(merged)
    // Web-m2: clear both the tag and the detected IDs on accept.
    clearDetectedMount()
  }

  return (
    <div
      className={cn(
        'inline-flex items-center gap-1.5 rounded-full bg-primary/10 px-3 py-1',
        'text-xs font-medium text-primary border border-primary/20',
      )}
    >
      <FolderCheck className="h-3 w-3" />
      <span>{detectedMountTag}</span>
      <span className="text-primary/60">{t('mounts.applied')}</span>
      <button
        type="button"
        onClick={handleAccept}
        className="ml-1 rounded px-1 hover:bg-primary/20"
        aria-label={t('mounts.bind')}
      >
        {t('common.ok')}
      </button>
      <button
        type="button"
        onClick={handleDismiss}
        className="ml-0.5 rounded p-0.5 hover:bg-primary/20"
        aria-label={t('common.dismiss')}
      >
        <X className="h-3 w-3" />
      </button>
    </div>
  )
}
