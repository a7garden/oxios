import { Search, Zap } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { useMemorySemanticSearch } from '@/hooks/use-memory'
import type { SemanticSearchResult } from '@/types/memory'

export function MemorySearch() {
  const { t } = useTranslation()
  const [query, setQuery] = useState('')
  const [results, setResults] = useState<SemanticSearchResult[]>([])
  const [engine, setEngine] = useState<string | null>(null)
  const semanticSearch = useMemorySemanticSearch()

  const handleSearch = async () => {
    if (!query.trim()) return
    // Semantic search falls back to keyword internally when the HNSW
    // index is unavailable (see oxios-memory manager::ops.rs), so a
    // single mode is sufficient and avoids the previously broken
    // "keyword" path that had no backend wiring.
    try {
      const res = await semanticSearch.mutateAsync({
        query,
        limit: 20,
      })
      setResults(res?.entries ?? [])
      setEngine(res?.engine ?? null)
    } catch {
      setResults([])
    }
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between gap-2">
        <Badge variant="secondary" className="gap-1">
          <Zap className="h-3 w-3" /> {t('memory.semanticSearch')}
          {engine && (
            <span className="text-xs font-normal text-muted-foreground/70">· {engine}</span>
          )}
        </Badge>
        {semanticSearch.isError && (
          <p className="text-xs text-destructive">
            {t('memory.searchFailed', 'Search failed. Please try again.')}
          </p>
        )}
      </div>
      <div className="flex gap-2">
        <div className="relative flex-1">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
          <Input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={t('memory.searchPlaceholder')}
            className="pl-9"
            onKeyDown={(e) => e.key === 'Enter' && handleSearch()}
          />
        </div>
        <Button onClick={handleSearch} disabled={semanticSearch.isPending}>
          {t('common.search')}
        </Button>
      </div>
      {results.length > 0 && (
        <div className="space-y-3">
          {results.map((r) => (
            <div key={r.id} className="border rounded-lg p-3 space-y-1">
              <div className="flex items-center justify-between">
                <Badge variant="secondary" className="text-xs">
                  {r.memory_type}
                </Badge>
                {r.score != null && (
                  <span className="text-xs text-muted-foreground">
                    {t('memory.relevance')}: {(r.score * 100).toFixed(1)}%
                  </span>
                )}
              </div>
              <p className="text-sm line-clamp-3">{r.content}</p>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
