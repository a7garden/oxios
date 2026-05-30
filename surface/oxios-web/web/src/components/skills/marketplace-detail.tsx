import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { ArrowLeft, Download, X } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { useToast } from '@/components/ui/sonner'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { LoadingCards } from '@/components/shared/loading'
import { ErrorState } from '@/components/shared/error-state'
import { api } from '@/lib/api-client'
import type { ClawHubSkillDetail as SkillDetailType } from '@/types'

interface MarketplaceDetailProps {
  slug: string
  onClose: () => void
}

export function MarketplaceDetail({ slug, onClose }: MarketplaceDetailProps) {
  const { t } = useTranslation()
  const qc = useQueryClient()
  const { toast } = useToast()

  const {
    data: detail,
    isLoading,
    isError,
    refetch,
  } = useQuery({
    queryKey: ['marketplace', 'detail', slug],
    queryFn: async () => {
      const res = await api.get<SkillDetailType>(`/api/marketplace/skills/${encodeURIComponent(slug)}`)
      return res
    },
    enabled: !!slug,
  })

  const installMutation = useMutation({
    mutationFn: () =>
      api.post(`/api/marketplace/skills/${encodeURIComponent(slug)}/install`, {
        version: detail?.latestVersion?.version,
      }),
    onSuccess: () => {
      toast(t('skills.installSuccess', { slug }), 'success')
      qc.invalidateQueries({ queryKey: ['skills'] })
    },
    onError: (err: unknown) => {
      toast(err instanceof Error ? err.message : t('skills.installFailed'), 'destructive')
    },
  })

  if (isLoading) return <LoadingCards count={2} />
  if (isError) return <ErrorState onRetry={() => refetch()} />
  if (!detail || !detail.skill) {
    return (
      <div className="flex items-center justify-center py-8 text-muted-foreground">
        {t('skills.noResults')}
      </div>
    )
  }

  const { skill, latestVersion, owner } = detail
  const displayName = skill.displayName || slug

  return (
    <div className="space-y-5">
      {/* Header */}
      <div className="flex items-start justify-between gap-3">
        <div className="flex items-start gap-3 min-w-0">
          <Button variant="ghost" size="icon" className="shrink-0 -ml-2" onClick={onClose}>
            <ArrowLeft className="h-4 w-4" />
          </Button>
          <div className="min-w-0">
            <h2 className="text-lg font-semibold leading-tight">{displayName}</h2>
            {skill.summary && (
              <p className="text-sm text-muted-foreground mt-1">{skill.summary}</p>
            )}
          </div>
        </div>
        <Button variant="ghost" size="icon" className="shrink-0" onClick={onClose}>
          <X className="h-4 w-4" />
        </Button>
      </div>

      {/* Meta */}
      <div className="flex flex-wrap items-center gap-2">
        <Badge variant="secondary" className="text-xs">OpenClaw</Badge>
        {latestVersion && (
          <Badge variant="outline" className="text-xs font-mono">
            v{latestVersion.version}
          </Badge>
        )}
        <span className="text-xs font-mono text-muted-foreground">{slug}</span>
      </div>

      {/* Owner */}
      {owner && (owner.handle || owner.displayName) && (
        <div className="rounded-md border px-3 py-2">
          <p className="text-xs font-medium text-muted-foreground uppercase tracking-wider mb-1">
            {t('skills.by')}
          </p>
          <div className="flex items-center gap-2">
            {owner.image && (
              <img src={owner.image} alt="" className="h-6 w-6 rounded-full" />
            )}
            <span className="text-sm">{owner.displayName || owner.handle}</span>
            {owner.handle && owner.displayName && (
              <span className="text-xs text-muted-foreground">@{owner.handle}</span>
            )}
          </div>
        </div>
      )}

      {/* Version info */}
      {latestVersion && (
        <div className="rounded-md border px-3 py-2 space-y-1">
          <p className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
            {t('skills.version')}
          </p>
          <div className="text-sm font-mono">v{latestVersion.version}</div>
          {latestVersion.changelog && (
            <p className="text-sm text-muted-foreground whitespace-pre-wrap">{latestVersion.changelog}</p>
          )}
        </div>
      )}

      {/* Metadata */}
      {detail.metadata && (
        <div className="rounded-md border px-3 py-2 space-y-1">
          {detail.metadata.os && detail.metadata.os.length > 0 && (
            <div className="text-sm">
              <span className="text-muted-foreground">OS:</span>{' '}
              {detail.metadata.os.join(', ')}
            </div>
          )}
          {detail.metadata.systems && detail.metadata.systems.length > 0 && (
            <div className="text-sm">
              <span className="text-muted-foreground">Systems:</span>{' '}
              {detail.metadata.systems.join(', ')}
            </div>
          )}
        </div>
      )}

      {/* Tags */}
      {skill.tags && Object.keys(skill.tags).length > 0 && (
        <div className="flex flex-wrap gap-1.5">
          {Object.entries(skill.tags).map(([key, value]) => (
            <Badge key={key} variant="outline" className="text-xs">
              {key}: {value}
            </Badge>
          ))}
        </div>
      )}

      {/* Install button */}
      <div className="pt-2 border-t">
        <Button
          className="w-full gap-2"
          onClick={() => installMutation.mutate()}
          disabled={installMutation.isPending}
        >
          <Download className="h-4 w-4" />
          {installMutation.isPending ? t('common.loading') : t('skills.install')}
        </Button>
      </div>
    </div>
  )
}
