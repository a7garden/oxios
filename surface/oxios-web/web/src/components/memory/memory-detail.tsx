import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Pin, Trash2 } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import type { MemoryDetail as MemDetail } from '@/types/memory'
import { TierBadge } from './tier-badge'
import { ProtectionBadge } from './protection-badge'
import { TypeBadge } from './type-badge'
import { useMemoryPin, useMemoryDelete } from '@/hooks/use-memory'

interface MemoryDetailProps {
  memory: MemDetail | null
  open: boolean
  onClose: () => void
}

export function MemoryDetail({ memory, open, onClose }: MemoryDetailProps) {
  const { t } = useTranslation()
  const pinMut = useMemoryPin()
  const deleteMut = useMemoryDelete()

  if (!memory) return null

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="max-w-lg max-h-[85vh] overflow-y-auto">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <TypeBadge type={memory.memory_type || 'fact'} />
            {memory.tier && <TierBadge tier={memory.tier} />}
            <Button
              variant="ghost"
              size="icon"
              className="ml-auto h-8 w-8"
              onClick={() =>
                pinMut.mutate({ id: memory.id, pinned: !memory.pinned })
              }
            >
              <Pin
                className={`h-4 w-4 ${memory.pinned ? 'fill-current' : ''}`}
              />
            </Button>
          </DialogTitle>
        </DialogHeader>
        <div className="mt-4 space-y-4">
          <div className="grid gap-2 text-sm">
            <div className="flex justify-between">
              <span className="text-muted-foreground">ID</span>
              <span className="font-mono text-xs">{memory.id}</span>
            </div>
            {memory.key && (
              <div className="flex justify-between">
                <span className="text-muted-foreground">Key</span>
                <span className="font-mono text-xs">{memory.key}</span>
              </div>
            )}
            {memory.project_ids && (
              <div className="flex justify-between">
                <span className="text-muted-foreground">
                  {t('memory.source')}
                </span>
                <span className="text-xs">{memory.project_ids}</span>
              </div>
            )}
            {memory.created_at && (
              <div className="flex justify-between">
                <span className="text-muted-foreground">
                  {t('memory.created')}
                </span>
                <span>{new Date(memory.created_at).toLocaleString()}</span>
              </div>
            )}
            {memory.updated_at && (
              <div className="flex justify-between">
                <span className="text-muted-foreground">Updated</span>
                <span>{new Date(memory.updated_at).toLocaleString()}</span>
              </div>
            )}
            {memory.last_accessed && (
              <div className="flex justify-between">
                <span className="text-muted-foreground">Last accessed</span>
                <span>{new Date(memory.last_accessed).toLocaleString()}</span>
              </div>
            )}
            {memory.access_count != null && (
              <div className="flex justify-between">
                <span className="text-muted-foreground">
                  {t('memory.appearances')}
                </span>
                <span>{memory.access_count}</span>
              </div>
            )}
            {memory.protected && (
              <div className="flex justify-between items-center">
                <span className="text-muted-foreground">
                  {t('memory.protection')}
                </span>
                <ProtectionBadge
                  level={memory.protection_reason || 'none'}
                />
              </div>
            )}
          </div>
          <div className="rounded-lg bg-muted p-3">
            <p className="text-sm whitespace-pre-wrap">
              {memory.content}
            </p>
          </div>
          {memory.summary && (
            <div className="rounded-lg border p-3">
              <p className="text-xs text-muted-foreground mb-1">Summary</p>
              <p className="text-sm">{memory.summary}</p>
            </div>
          )}
          {memory.tags && memory.tags.length > 0 && (
            <div className="flex flex-wrap gap-1">
              {memory.tags.map((tag) => (
                <span
                  key={tag}
                  className="text-xs bg-secondary px-2 py-0.5 rounded"
                >
                  {tag}
                </span>
              ))}
            </div>
          )}
          <div className="flex gap-2 pt-2">
            <Button
              variant="destructive"
              size="sm"
              onClick={() => {
                deleteMut.mutate(memory.id)
                onClose()
              }}
            >
              <Trash2 className="h-4 w-4 mr-1" /> {t('memory.deleteMemory')}
            </Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  )
}
