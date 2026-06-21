import { Calendar, Hash, Link2 } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import type { MemoryMapEntry } from '@/types/memory'
import { TierBadge } from './tier-badge'
import { TypeBadge } from './type-badge'

interface SelectionPanelProps {
  selected: MemoryMapEntry | null
  /** All entries — used to resolve neighbour ids → labels. */
  allEntries: MemoryMapEntry[]
  onOpenDetail?: (id: string) => void
  onHoverNeighbour?: (id: string | null) => void
}

/**
 * Right-side panel showing details for the currently selected map node.
 *
 * Renders nothing when no node is selected so the parent can mount it
 * in a flex/grid slot without reserving space.
 */
export function SelectionPanel({
  selected,
  allEntries,
  onOpenDetail,
  onHoverNeighbour,
}: SelectionPanelProps) {
  const { t } = useTranslation()

  if (!selected) {
    return (
      <div
        className="flex h-full items-center justify-center rounded-md border border-dashed p-6 text-sm text-muted-foreground"
        data-testid="selection-panel-empty"
      >
        {t('memory.mapSelectPrompt')}
      </div>
    )
  }

  const entryById = new Map(allEntries.map((e) => [e.id, e]))

  return (
    <Card className="h-full overflow-hidden" data-testid="selection-panel">
      <CardHeader className="pb-3">
        <CardTitle className="flex flex-wrap items-center gap-2 text-base">
          <TypeBadge type={selected.mem_type || 'fact'} />
          <TierBadge tier={selected.tier || 'warm'} />
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-3 overflow-y-auto pb-4 text-sm">
        <p className="whitespace-pre-wrap break-words">
          {selected.content_preview || t('memory.noPreview')}
        </p>

        <dl className="grid grid-cols-2 gap-2 text-xs">
          <div className="flex items-center gap-1.5 text-muted-foreground">
            <Hash className="h-3 w-3" />
            <dt className="sr-only">{t('memory.id')}</dt>
            <dd className="truncate font-mono">{selected.id}</dd>
          </div>
          <div className="flex items-center gap-1.5 text-muted-foreground">
            <Calendar className="h-3 w-3" />
            <dt className="sr-only">{t('memory.createdAt')}</dt>
            <dd>{formatDate(selected.created_at)}</dd>
          </div>
        </dl>

        {selected.top_neighbors.length > 0 ? (
          <div>
            <h4 className="mb-1.5 flex items-center gap-1.5 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              <Link2 className="h-3 w-3" />
              {t('memory.mapRelated', { count: selected.top_neighbors.length })}
            </h4>
            <ul className="space-y-1">
              {selected.top_neighbors.map((nbr) => {
                const nbrEntry = entryById.get(nbr.id)
                return (
                  <li
                    key={nbr.id}
                    className="flex items-center justify-between gap-2 rounded-sm border bg-muted/40 px-2 py-1 text-xs"
                    onMouseEnter={() => onHoverNeighbour?.(nbr.id)}
                    onMouseLeave={() => onHoverNeighbour?.(null)}
                  >
                    <span className="truncate">{nbrEntry?.content_preview ?? nbr.id}</span>
                    <span className="shrink-0 font-mono text-muted-foreground">
                      {nbr.similarity.toFixed(2)}
                    </span>
                  </li>
                )
              })}
            </ul>
          </div>
        ) : (
          <p className="text-xs text-muted-foreground">{t('memory.mapNoRelated')}</p>
        )}

        <div className="flex gap-2 pt-1">
          <Button
            size="sm"
            variant="default"
            onClick={() => onOpenDetail?.(selected.id)}
            data-testid="selection-open-detail"
          >
            {t('memory.mapOpenDetail')}
          </Button>
        </div>
      </CardContent>
    </Card>
  )
}

function formatDate(iso: string): string {
  if (!iso) return ''
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return iso
  return d.toLocaleString()
}
