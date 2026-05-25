import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { Search, Store } from 'lucide-react'
import { useCallback, useDeferredValue, useMemo, useState } from 'react'
import { EmptyState } from '@/components/shared/empty-state'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { useToast } from '@/components/ui/sonner'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { api } from '@/lib/api-client'
import type { ClawHubSearchResult } from '@/types'

export const Route = createFileRoute('/marketplace')({ component: MarketplacePage })

function MarketplacePage() {
  const [query, setQuery] = useState('')
  const deferredQuery = useDeferredValue(query)
  const queryClient = useQueryClient()

  const {
    data: results,
    isLoading,
    isError,
    refetch,
  } = useQuery({
    queryKey: ['marketplace', 'search', deferredQuery],
    queryFn: async () => {
      const res = await api.get<{ results: ClawHubSearchResult[] }>('/api/marketplace/search', {
        q: deferredQuery,
      })
      return res.results ?? []
    },
    enabled: deferredQuery.trim().length > 0,
    refetchOnWindowFocus: false,
  })

  const { toast } = useToast()

  const installMutation = useMutation({
    mutationFn: ({ slug, version }: { slug: string; version?: string }) =>
      api.post('/api/marketplace/skills/' + slug + '/install', { version }),
    onSuccess: (_data, variables) => {
      toast(`Installed "${variables.slug}" successfully.`, 'success')
      queryClient.invalidateQueries({ queryKey: ['skills'] })
    },
    onError: (err: unknown, _variables) => {
      const message =
        err instanceof Error ? err.message : 'Installation failed. Please try again.'
      toast(message, 'destructive')
    },
  })

  const handleSearch = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    setQuery(e.target.value)
  }, [])

  const hasSearch = deferredQuery.trim().length > 0

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold flex items-center gap-2">
          <Store className="h-6 w-6" />
          Marketplace
        </h1>
        <p className="text-muted-foreground">
          Browse and install skills from ClawHub
        </p>
      </div>

      {/* Search */}
      <div className="relative">
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground pointer-events-none" />
        <Input
          placeholder="Search skills..."
          value={query}
          onChange={handleSearch}
          className="pl-10"
          autoFocus
        />
      </div>

      {/* Content */}
      {!hasSearch ? (
        <EmptyState
          icon={<Search className="h-10 w-10" />}
          title="Search the marketplace"
          description="Type a query above to search for skills on ClawHub."
        />
      ) : isLoading ? (
        <LoadingCards count={4} />
      ) : isError ? (
        <ErrorState onRetry={() => refetch()} />
      ) : results?.length === 0 ? (
        <EmptyState
          icon={<Search className="h-10 w-10" />}
          title="No results"
          description={`No skills found for "${deferredQuery}". Try a different query.`}
        />
      ) : (
        <div className="grid gap-4">
          {results!.map((skill) => (
            <MarketplaceCard
              key={skill.slug}
              skill={skill}
              isInstalling={installMutation.isPending}
              onInstall={(slug, version) => installMutation.mutate({ slug, version })}
            />
          ))}
        </div>
      )}
    </div>
  )
}

function MarketplaceCard({
  skill,
  isInstalling,
  onInstall,
}: {
  skill: ClawHubSearchResult
  isInstalling: boolean
  onInstall: (slug: string, version?: string) => void
}) {
  const version = skill.version
  const displayName = skill.displayName || skill.slug
  const summary = skill.summary || ''

  const relativeTime = useMemo(() => {
    if (!skill.updatedAt) return null
    const diff = Date.now() - skill.updatedAt
    const days = Math.floor(diff / 86_400_000)
    if (days === 0) return 'today'
    if (days === 1) return '1d ago'
    if (days < 30) return `${days}d ago`
    const weeks = Math.floor(days / 7)
    if (weeks === 1) return '1w ago'
    if (weeks < 4) return `${weeks}w ago`
    const months = Math.floor(days / 30)
    return `${months}mo ago`
  }, [skill.updatedAt])

  return (
    <Card className="transition-shadow hover:shadow-md">
      <CardContent className="p-5 space-y-3">
        {/* Header */}
        <div className="flex items-start justify-between gap-3">
          <div className="flex items-start gap-2 min-w-0">
            <span className="text-lg leading-none mt-0.5 shrink-0">🔍</span>
            <div className="min-w-0">
              <h3 className="font-semibold text-base leading-tight">{displayName}</h3>
              {summary && (
                <p className="text-sm text-muted-foreground mt-0.5 line-clamp-2">{summary}</p>
              )}
            </div>
          </div>
          <div className="flex items-center gap-2 shrink-0">
            {version && (
              <Badge variant="outline" className="text-xs font-mono">
                v{version}
              </Badge>
            )}
            <Button
              size="sm"
              variant="default"
              onClick={() => onInstall(skill.slug, version)}
              disabled={isInstalling}
              className="gap-1.5"
            >
              Install
            </Button>
          </div>
        </div>

        {/* Meta */}
        <div className="flex items-center gap-2 text-xs text-muted-foreground">
          <span className="font-mono text-muted-foreground/80">{skill.slug}</span>
          {relativeTime && (
            <>
              <span>·</span>
              <span>{relativeTime}</span>
            </>
          )}
        </div>
      </CardContent>
    </Card>
  )
}