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
  const [mode, setMode] = useState<'keyword' | 'semantic'>('keyword')
  const [results, setResults] = useState<SemanticSearchResult[]>([])
  const semanticSearch = useMemorySemanticSearch()

  const handleSearch = async () => {
    if (!query.trim()) return
    if (mode === 'semantic') {
      const res = await semanticSearch.mutateAsync({
        query,
        limit: 20,
      })
      setResults(res?.entries ?? [])
    }
  }

  return (
    <div className="space-y-4">
      <div className="flex gap-2">
        <Button
          variant={mode === 'keyword' ? 'default' : 'outline'}
          size="sm"
          onClick={() => setMode('keyword')}
        >
          <Search className="h-4 w-4 mr-1" /> {t('memory.keywordSearch')}
        </Button>
        <Button
          variant={mode === 'semantic' ? 'default' : 'outline'}
          size="sm"
          onClick={() => setMode('semantic')}
        >
          <Zap className="h-4 w-4 mr-1" /> {t('memory.semanticSearch')}
        </Button>
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
