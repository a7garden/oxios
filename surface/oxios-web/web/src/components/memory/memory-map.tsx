import { useEffect, useMemo, useState, useCallback, useRef } from 'react'
import { useTranslation } from 'react-i18next'
import { useMemoryMap } from '@/hooks/use-memory'
import { Select } from '@/components/ui/select'
import { Switch } from '@/components/ui/switch'
import { Input } from '@/components/ui/input'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { EmptyState } from '@/components/shared/empty-state'
import { EmbeddingCanvas } from './embedding-canvas'
import { SelectionPanel } from './selection-panel'
import { ClusterLegend } from './cluster-legend'
import { MemoryDetail } from './memory-detail'
import { Brain, Search } from 'lucide-react'
import { useMemoryDetail } from '@/hooks/use-memory'
import type { MemoryMapEntry, MemoryDetail as MemDetail } from '@/types/memory'

/**
 * Memory Embedding Map (RFC-T1-B).
 *
 * Top-level component for the "Map" tab on the Memory page. Owns:
 *  - filter / search state
 *  - the data fetch (`useMemoryMap`)
 *  - selection + detail modal state
 *  - Cmd+F focus shortcut
 *
 * Renders a two-column grid: canvas + legend on the left, selection
 * panel on the right. The detail modal is shared with the Browse tab.
 */
export function MemoryMap() {
  const { t } = useTranslation()
  const [tier, setTier] = useState<string>('all')
  const [type, setType] = useState<string>('all')
  const [animate, setAnimate] = useState<boolean>(true)
  const [query, setQuery] = useState<string>('')
  const [activeQuery, setActiveQuery] = useState<string>('')
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [hoveredId, setHoveredId] = useState<string | null>(null)
  const [flyToId, setFlyToId] = useState<string | null>(null)
  const [detailId, setDetailId] = useState<string | null>(null)
  const [detailOpen, setDetailOpen] = useState<boolean>(false)

  const {
    data,
    isLoading,
    isError,
    refetch,
  } = useMemoryMap({
    tier: tier === 'all' ? undefined : tier,
    mem_type: type === 'all' ? undefined : type,
    limit: 500,
  })

  const entries = useMemo<MemoryMapEntry[]>(() => data?.entries ?? [], [data])

  // Find the currently selected entry (or fall back to the hovered one)
  // for the side panel.
  const panelEntry = useMemo(() => {
    const id = selectedId ?? hoveredId
    if (!id) return null
    return entries.find((e) => e.id === id) ?? null
  }, [selectedId, hoveredId, entries])

  // Cmd+F (or Ctrl+F) focuses the search input.
  const searchInputRef = useRef<HTMLInputElement | null>(null)
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const isFind = (e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'f'
      if (isFind && searchInputRef.current) {
        e.preventDefault()
        searchInputRef.current.focus()
        searchInputRef.current.select()
      } else if (
        e.key === 'Escape' &&
        searchInputRef.current &&
        document.activeElement === searchInputRef.current
      ) {
        setQuery('')
        setActiveQuery('')
        setFlyToId(null)
        searchInputRef.current.blur()
      }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [])

  // Submitting the search finds the first matching node, highlights it,
  // and flies the camera to it.
  const runSearch = useCallback(() => {
    const q = query.trim().toLowerCase()
    setActiveQuery(q)
    if (!q) {
      setFlyToId(null)
      return
    }
    const hit = entries.find(
      (e) =>
        e.id.toLowerCase().includes(q) ||
        e.content_preview.toLowerCase().includes(q) ||
        e.mem_type.toLowerCase().includes(q),
    )
    if (hit) {
      setSelectedId(hit.id)
      setFlyToId(hit.id)
      // Clear the fly-to flag after the camera transition completes so
      // a follow-up identical search re-runs.
      setTimeout(() => setFlyToId((prev) => (prev === hit.id ? null : prev)), 700)
    }
  }, [query, entries])

  // Detail modal: load the full memory entry when a user opens one.
  const detailQuery = useMemoryDetail(detailId)
  const detailEntry: MemDetail | null = useMemo(() => {
    if (!detailQuery.data) return null
    const e = detailQuery.data
    return {
      id: e.id,
      key: e.key,
      tier: e.tier,
      memory_type: e.memory_type,
      content: e.content,
      summary: e.summary,
      project_ids: e.project_ids,
      created_at: e.created_at,
      updated_at: e.updated_at,
      last_accessed: e.last_accessed,
      access_count: e.access_count,
      pinned: e.pinned,
      protected: e.protected,
      protection_reason: e.protection_reason,
      tags: e.tags,
      metadata: e.metadata,
    }
  }, [detailQuery.data])

  const handleOpenDetail = useCallback((id: string) => {
    setDetailId(id)
    setDetailOpen(true)
  }, [])

  if (isError) {
    return <ErrorState onRetry={() => refetch()} />
  }

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
  ]

  return (
    <div className="space-y-3" data-testid="memory-map">
      <div className="flex flex-wrap items-center gap-2">
        <Select
          value={tier}
          onValueChange={setTier}
          options={tierOptions}
          placeholder={t('memory.filterByTier')}
          className="w-full sm:w-32"
          data-testid="map-tier-filter"
        />
        <Select
          value={type}
          onValueChange={setType}
          options={typeOptions}
          placeholder={t('memory.filterByType')}
          className="w-full sm:w-40"
          data-testid="map-type-filter"
        />
        <div className="flex items-center gap-1.5">
          <Switch
            checked={animate}
            onCheckedChange={setAnimate}
            data-testid="map-animate-toggle"
            aria-label={t('memory.mapAnimate')}
          />
          <span className="text-xs text-muted-foreground">{t('memory.mapAnimate')}</span>
        </div>
        <form
          className="ml-auto flex items-center gap-1.5"
          onSubmit={(e) => {
            e.preventDefault()
            runSearch()
          }}
        >
          <div className="relative">
            <Search className="pointer-events-none absolute left-2 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted-foreground" />
            <Input
              ref={searchInputRef}
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={t('memory.mapSearchPlaceholder')}
              className="h-8 w-48 pl-7 text-xs"
              data-testid="map-search-input"
            />
          </div>
        </form>
      </div>

      <div className="grid grid-cols-1 gap-3 lg:grid-cols-[1fr_320px]">
        <div className="space-y-2">
          <EmbeddingCanvas
            entries={entries}
            selectedId={selectedId ?? hoveredId}
            onHover={setHoveredId}
            onSelect={(id) => setSelectedId(id)}
            flyToId={flyToId}
            animate={animate}
          />
          <div className="flex flex-wrap items-center gap-2">
            <ClusterLegend />
            <p className="text-xs text-muted-foreground">
              {t('memory.mapNodeCount', { count: entries.length })}
              {activeQuery
                ? ` · ${t('memory.mapSearchActive', { query: activeQuery })}`
                : ''}
            </p>
          </div>
        </div>
        <div className="min-h-[360px]">
          {isLoading ? (
            <LoadingCards count={1} />
          ) : entries.length === 0 ? (
            <EmptyState
              icon={<Brain className="h-10 w-10" />}
              title={t('memory.mapEmpty')}
              description={t('memory.mapEmptyDescription')}
            />
          ) : (
            <SelectionPanel
              selected={panelEntry}
              allEntries={entries}
              onOpenDetail={handleOpenDetail}
              onHoverNeighbour={setHoveredId}
            />
          )}
        </div>
      </div>

      <MemoryDetail memory={detailEntry} open={detailOpen} onClose={() => setDetailOpen(false)} />
    </div>
  )
}
