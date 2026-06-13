import { Clock, Pin } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Card, CardContent } from '@/components/ui/card'
import type { MemoryDetail } from '@/types/memory'
import { TierBadge } from './tier-badge'
import { TypeBadge } from './type-badge'

interface MemoryCardProps {
  memory: MemoryDetail
  onClick: () => void
}

export function MemoryCard({ memory, onClick }: MemoryCardProps) {
  const { t } = useTranslation()
  return (
    <Card
      className="cursor-pointer hover:border-primary/30 hover:shadow-sm transition-all"
      onClick={onClick}
    >
      <CardContent className="p-4 space-y-2">
        <div className="flex items-center justify-between">
          <div className="flex gap-1.5">
            <TypeBadge type={memory.memory_type || 'fact'} />
            {memory.tier && <TierBadge tier={memory.tier} />}
          </div>
          {memory.pinned && <Pin className="h-3 w-3 text-muted-foreground" />}
        </div>
        <p className="text-sm line-clamp-2 break-words">
          {memory.content?.slice(0, 120) || memory.key}
        </p>
        <div className="flex items-center justify-between text-xs text-muted-foreground">
          <div className="flex items-center gap-1">
            <Clock className="h-3 w-3" />
            {memory.created_at ? new Date(memory.created_at).toLocaleDateString() : ''}
          </div>
          {memory.access_count != null && (
            <span>
              {t('memory.appearances')}: {memory.access_count}
            </span>
          )}
        </div>
      </CardContent>
    </Card>
  )
}
