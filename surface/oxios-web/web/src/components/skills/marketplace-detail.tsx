import { useQuery } from '@tanstack/react-query'
import { ExternalLink, Package } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Separator } from '@/components/ui/separator'
import { Skeleton } from '@/components/ui/skeleton'
import { api } from '@/lib/api-client'
import type { ClawHubSkillDetail } from '@/types'

// ─── Marketplace Detail side panel ─────────────────────────

export function MarketplaceDetail({ slug, onClose }: { slug: string; onClose: () => void }) {
  const { t } = useTranslation()

  const { data, isLoading, isError } = useQuery({
    queryKey: ['marketplace', 'skill', slug],
    queryFn: async () => {
      const r = await api.get<ClawHubSkillDetail>(`/api/marketplace/skills/${slug}`)
      return r
    },
    staleTime: 60_000, // 1 min
  })

  return (
    <div className="space-y-4">
      {/* Header */}
      <div className="flex items-start justify-between gap-2">
        <div className="flex items-center gap-2 min-w-0">
          <Package className="h-5 w-5 text-muted-foreground shrink-0" />
          <div className="min-w-0">
            <h2 className="font-semibold text-lg leading-tight truncate">
              {isLoading ? <Skeleton className="h-5 w-32" /> : data?.skill?.displayName ?? slug}
            </h2>
            <Badge variant="secondary" className="text-xs mt-1">OpenClaw</Badge>
          </div>
        </div>
        <Button variant="ghost" size="icon" className="shrink-0 h-7 w-7" onClick={onClose}>
          {/* X icon inline to avoid import from parent */}
          <svg className="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
          </svg>
        </Button>
      </div>

      {isLoading ? (
        <div className="space-y-3">
          <Skeleton className="h-4 w-full" />
          <Skeleton className="h-4 w-3/4" />
          <Skeleton className="h-4 w-1/2" />
          <Skeleton className="h-20 w-full" />
        </div>
      ) : isError ? (
        <p className="text-sm text-destructive">{t('common.error')}</p>
      ) : data ? (
        <>
          {/* Summary */}
          {data.skill?.summary && (
            <p className="text-sm text-muted-foreground">{data.skill.summary}</p>
          )}

          {/* Version + date */}
          <div className="flex items-center gap-2 text-xs text-muted-foreground">
            {data.latestVersion && (
              <span className="font-mono">v{data.latestVersion.version}</span>
            )}
            {data.skill?.updatedAt && (
              <>
                <span>·</span>
                <span>{formatDate(data.skill.updatedAt)}</span>
              </>
            )}
          </div>

          {/* Owner */}
          {data.owner && (
            <div className="flex items-center gap-2 text-xs text-muted-foreground">
              {data.owner.image && (
                <img src={data.owner.image} alt="" className="h-5 w-5 rounded-full" />
              )}
              {data.owner.displayName && <span>{data.owner.displayName}</span>}
              {data.owner.handle && <span className="font-mono">@{data.owner.handle}</span>}
            </div>
          )}

          <Separator />

          {/* Tags */}
          {data.skill?.tags && Object.keys(data.skill.tags).length > 0 && (
            <div className="space-y-2">
              <p className="text-xs font-medium text-muted-foreground uppercase tracking-wider">Tags</p>
              <div className="flex flex-wrap gap-1.5">
                {Object.entries(data.skill.tags).map(([key, val]) => (
                  <Badge key={key} variant="outline" className="text-xs">
                    {key}: {val}
                  </Badge>
                ))}
              </div>
            </div>
          )}

          {/* Metadata */}
          {(data.metadata?.os || data.metadata?.systems) && (
            <div className="space-y-2">
              <p className="text-xs font-medium text-muted-foreground uppercase tracking-wider">Compatibility</p>
              <div className="flex flex-wrap gap-1.5">
                {data.metadata?.os?.map(o => (
                  <Badge key={o} variant="outline" className="text-xs">{o}</Badge>
                ))}
                {data.metadata?.systems?.map(s => (
                  <Badge key={s} variant="secondary" className="text-xs">{s}</Badge>
                ))}
              </div>
            </div>
          )}

          {/* Changelog */}
          {data.latestVersion?.changelog && (
            <div className="space-y-2">
              <p className="text-xs font-medium text-muted-foreground uppercase tracking-wider">Changelog</p>
              <div className="rounded-md bg-muted/50 px-3 py-2 text-xs text-muted-foreground whitespace-pre-wrap font-mono max-h-40 overflow-y-auto">
                {data.latestVersion.changelog}
              </div>
            </div>
          )}

          {/* Slug + ClawHub link */}
          <div className="space-y-1">
            <p className="text-xs text-muted-foreground/60 font-mono truncate" title={slug}>{slug}</p>
            <a
              href={`https://clawhub.io/skills/${slug}`}
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex items-center gap-1 text-xs text-primary hover:underline"
            >
              <ExternalLink className="h-3 w-3" /> View on ClawHub
            </a>
          </div>
        </>
      ) : null}
    </div>
  )
}

// ─── Helpers ─────────────────────────────────────────────────

function formatDate(ts: number): string {
  return new Date(ts).toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' })
}