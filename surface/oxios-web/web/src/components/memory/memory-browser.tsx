import { Brain } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { EmptyState } from '@/components/shared/empty-state'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { Select } from '@/components/ui/select'
import { useMemoryList } from '@/hooks/use-memory'
import type { MemoryDetail } from '@/types/memory'
import { MemoryCard } from './memory-card'

interface MemoryBrowserProps {
  onSelect: (memory: MemoryDetail) => void
}

export function MemoryBrowser({ onSelect }: MemoryBrowserProps) {
  const { t } = useTranslation()
  const [tier, setTier] = useState<string>('all')
  const [type, setType] = useState<string>('all')
  const { data, isLoading, isError, refetch } = useMemoryList(
    tier !== 'all' ? tier : undefined,
    type !== 'all' ? type : undefined,
  )

  if (isError) return <ErrorState onRetry={() => refetch()} />

  const items = data?.items ?? []

  const tierOptions = [
    { label: t('common.all'), value: 'all' },
    { label: t('memory.hot'), value: 'hot' },
    { label: t('memory.warm'), value: 'warm' },
    { label: t('memory.cold'), value: 'cold' },
  ]

  const typeOptions = [
    { label: t('common.all'), value: 'all' },
    { label: t('memory.fact'), value: 'fact' },
    { label: t('memory.episode'), value: 'episode' },
    { label: t('memory.knowledge'), value: 'knowledge' },
    { label: t('memory.decision'), value: 'decision' },
    { label: t('memory.skill'), value: 'skill' },
    { label: t('memory.preference'), value: 'preference' },
    { label: t('memory.conversation'), value: 'conversation' },
    { label: t('memory.session'), value: 'session' },
    { label: t('memory.procedure'), value: 'procedure' },
  ]

  return (
    <div className="space-y-4">
      <div className="flex gap-3 flex-wrap">
        <Select
          value={tier}
          onValueChange={setTier}
          options={tierOptions}
          placeholder={t('memory.filterByTier')}
          className="w-full sm:w-40"
        />
        <Select
          value={type}
          onValueChange={setType}
          options={typeOptions}
          placeholder={t('memory.filterByType')}
          className="w-full sm:w-40"
        />
      </div>
      {isLoading ? (
        <LoadingCards count={6} />
      ) : items.length === 0 ? (
        <EmptyState
          icon={<Brain className="h-10 w-10" />}
          title={t('memory.noMemories')}
          description={t('memory.description')}
        />
      ) : (
        <div className="grid gap-3 md:grid-cols-2 lg:grid-cols-3">
          {items.map((m) => (
            <MemoryCard key={m.id} memory={m} onClick={() => onSelect(m)} />
          ))}
        </div>
      )}
    </div>
  )
}
