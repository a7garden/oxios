import { createFileRoute } from '@tanstack/react-router'
import { useState } from 'react'
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
  const [activeTab, setActiveTab] = useState('overview')
  const [selected, setSelected] = useState<MemDetail | null>(null)
  const [detailOpen, setDetailOpen] = useState(false)

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">{t('memory.title')}</h1>
          <p className="text-muted-foreground">{t('memory.subtitle')}</p>
        </div>
      </div>
      <Tabs>
        <TabsList>
          <TabsTrigger
            data-state={activeTab === 'overview' ? 'active' : 'inactive'}
            onClick={() => setActiveTab('overview')}
          >
            {t('memory.overview')}
          </TabsTrigger>
          <TabsTrigger
            data-state={activeTab === 'browse' ? 'active' : 'inactive'}
            onClick={() => setActiveTab('browse')}
          >
            {t('memory.browse')}
          </TabsTrigger>
          <TabsTrigger
            data-state={activeTab === 'map' ? 'active' : 'inactive'}
            onClick={() => setActiveTab('map')}
            data-testid="memory-tab-map"
          >
            {t('memory.map')}
          </TabsTrigger>
          <TabsTrigger
            data-state={activeTab === 'dream' ? 'active' : 'inactive'}
            onClick={() => setActiveTab('dream')}
          >
            {t('memory.dream')}
          </TabsTrigger>
          <TabsTrigger
            data-state={activeTab === 'search' ? 'active' : 'inactive'}
            onClick={() => setActiveTab('search')}
          >
            {t('memory.search')}
          </TabsTrigger>
        </TabsList>
        {activeTab === 'overview' && (
          <TabsContent value="overview">
            <MemoryOverview />
          </TabsContent>
        )}
        {activeTab === 'browse' && (
          <TabsContent value="browse">
            <MemoryBrowser
              onSelect={(m) => {
                setSelected(m)
                setDetailOpen(true)
              }}
            />
          </TabsContent>
        )}
        {activeTab === 'map' && (
          <TabsContent value="map">
            <MemoryMap />
          </TabsContent>
        )}
        {activeTab === 'dream' && (
          <TabsContent value="dream">
            <DreamPanel />
          </TabsContent>
        )}
        {activeTab === 'search' && (
          <TabsContent value="search">
            <MemorySearch />
          </TabsContent>
        )}
      </Tabs>
      <MemoryDetail memory={selected} open={detailOpen} onClose={() => setDetailOpen(false)} />
    </div>
  )
}
