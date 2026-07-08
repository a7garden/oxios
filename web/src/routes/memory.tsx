import { createFileRoute } from '@tanstack/react-router'
import { useCallback, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { DreamPanel } from '@/components/memory/dream-panel'
import { MemoryBrowser } from '@/components/memory/memory-browser'
import { MemoryDetail } from '@/components/memory/memory-detail'
import { MemoryMap } from '@/components/memory/memory-map'
import { MemoryOverview } from '@/components/memory/memory-overview'
import { MemorySearch } from '@/components/memory/memory-search'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import type { MemoryDetail as MemDetail } from '@/types/memory'

export const Route = createFileRoute('/memory')({ component: MemoryPage })

function MemoryPage() {
  const { t } = useTranslation()
  const [selected, setSelected] = useState<MemDetail | null>(null)
  const [detailOpen, setDetailOpen] = useState(false)

  const handleSelectMemory = useCallback((m: MemDetail) => {
    setSelected(m)
    setDetailOpen(true)
  }, [])

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">{t('memory.title')}</h1>
          <p className="text-muted-foreground">{t('memory.subtitle')}</p>
        </div>
      </div>
      <Tabs defaultValue="overview">
        <TabsList>
          <TabsTrigger value="overview">{t('memory.overview')}</TabsTrigger>
          <TabsTrigger value="browse">{t('memory.browse')}</TabsTrigger>
          <TabsTrigger value="search">{t('memory.search')}</TabsTrigger>
          <TabsTrigger value="map" data-testid="memory-tab-map">
            {t('memory.map')}
          </TabsTrigger>
          <TabsTrigger value="dream">{t('memory.dream')}</TabsTrigger>
        </TabsList>
        <TabsContent value="overview">
          <MemoryOverview />
        </TabsContent>
        <TabsContent value="browse">
          <MemoryBrowser onSelect={handleSelectMemory} />
        </TabsContent>
        <TabsContent value="search">
          <MemorySearch />
        </TabsContent>
        <TabsContent value="map">
          <MemoryMap />
        </TabsContent>
        <TabsContent value="dream">
          <DreamPanel />
        </TabsContent>
      </Tabs>
      <MemoryDetail memory={selected} open={detailOpen} onClose={() => setDetailOpen(false)} />
    </div>
  )
}
