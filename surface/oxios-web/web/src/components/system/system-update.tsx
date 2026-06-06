import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import {
  ArrowDownToLine,
  CheckCircle2,
  ChevronDown,
  ChevronUp,
  ExternalLink,
  Loader2,
  AlertCircle,
} from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { api } from '@/lib/api-client'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Separator } from '@/components/ui/separator'
import type { UpdateCheckResponse, ChangelogResponse } from '@/types'

function formatDate(iso: string): string {
  if (!iso) return '-'
  try {
    return new Date(iso).toLocaleDateString(undefined, {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
    })
  } catch {
    return iso
  }
}

export function SystemUpdateCard() {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const [showNotes, setShowNotes] = useState(false)

  const {
    data: check,
    isLoading: checkLoading,
    isError: checkError,
    refetch: refetchCheck,
  } = useQuery({
    queryKey: ['update-check'],
    queryFn: () => api.get<UpdateCheckResponse>('/api/update/check'),
    staleTime: 5 * 60 * 1000,
  })

  const {
    data: changelog,
    isLoading: changelogLoading,
  } = useQuery({
    queryKey: ['update-changelog'],
    queryFn: () => api.get<ChangelogResponse>('/api/update/changelog'),
    staleTime: 5 * 60 * 1000,
    enabled: showNotes,
  })

  const updateMutation = useMutation({
    mutationFn: (): Promise<Record<string, unknown>> =>
      api.post('/api/update/run', { binary: true, web: true }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['update-check'] })
      queryClient.invalidateQueries({ queryKey: ['status'] })
    },
  })

  // Loading state
  if (checkLoading) {
    return (
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <ArrowDownToLine className="h-4 w-4" />
            {t('update.title')}
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="flex items-center gap-2 text-sm text-muted-foreground">
            <Loader2 className="h-4 w-4 animate-spin" />
            {t('update.checking')}
          </div>
        </CardContent>
      </Card>
    )
  }

  // Error state
  if (checkError || !check) {
    return (
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <ArrowDownToLine className="h-4 w-4" />
            {t('update.title')}
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2 text-sm text-destructive">
              <AlertCircle className="h-4 w-4" />
              {t('update.checkFailed')}
            </div>
            <Button variant="outline" size="sm" onClick={() => refetchCheck()}>
              {t('common.retry')}
            </Button>
          </div>
        </CardContent>
      </Card>
    )
  }

  const isUpToDate = !check.update_available

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <ArrowDownToLine className="h-4 w-4" />
          {t('update.title')}
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        {/* Version comparison */}
        <div className="grid grid-cols-2 gap-4">
          <div>
            <p className="text-xs text-muted-foreground">{t('update.currentVersion')}</p>
            <p className="text-lg font-mono font-semibold">{check.current_version}</p>
          </div>
          <div>
            <p className="text-xs text-muted-foreground">{t('update.latestVersion')}</p>
            <div className="flex items-center gap-2">
              <p className="text-lg font-mono font-semibold">{check.latest_version}</p>
              {isUpToDate ? (
                <Badge variant="success" className="text-xs">{t('update.upToDate')}</Badge>
              ) : (
                <Badge variant="destructive" className="text-xs">{t('update.updateAvailable')}</Badge>
              )}
            </div>
          </div>
        </div>

        <Separator />

        {/* Update available */}
        {!isUpToDate && (
          <>
            <div className="text-xs text-muted-foreground">
              {t('update.methodDescription')}
            </div>

            {/* Update button */}
            <div className="flex items-center gap-3">
              <Button
                onClick={() => updateMutation.mutate()}
                disabled={updateMutation.isPending}
                className="gap-2"
              >
                {updateMutation.isPending ? (
                  <>
                    <Loader2 className="h-4 w-4 animate-spin" />
                    {t('update.updating')}
                  </>
                ) : (
                  <>
                    <ArrowDownToLine className="h-4 w-4" />
                    {t('update.updateTo', { version: check.latest_version })}
                  </>
                )}
              </Button>
              <a
                href={check.html_url}
                target="_blank"
                rel="noopener noreferrer"
                className="text-xs text-muted-foreground hover:text-foreground flex items-center gap-1"
              >
                {t('update.viewOnGithub')}
                <ExternalLink className="h-3 w-3" />
              </a>
            </div>

            {/* Update result */}
            {updateMutation.isSuccess && updateMutation.data && (
              <div className="rounded-lg border border-success-subtle p-3 text-sm text-success">
                <div className="flex items-center gap-2 mb-1">
                  <CheckCircle2 className="h-4 w-4" />
                  <span className="font-medium">{t('update.updateSuccess')}</span>
                </div>
                <p className="text-xs">{(updateMutation.data as Record<string, unknown>)?.message as string}</p>
                {((updateMutation.data as Record<string, unknown>)?.binary_updated as boolean) && (
                  <p className="text-xs mt-1 text-warning">
                    {t('update.restartRequired')}
                  </p>
                )}
              </div>
            )}

            {updateMutation.isError && (
              <div className="rounded-lg border border-error-subtle p-3 text-sm text-error">
                <div className="flex items-center gap-2">
                  <AlertCircle className="h-4 w-4" />
                  <span>{t('update.updateFailed')}</span>
                </div>
                <p className="text-xs mt-1">{(updateMutation.error as Error)?.message || t('update.unknownError')}</p>
              </div>
            )}
          </>
        )}

        {/* Already up to date */}
        {isUpToDate && (
          <div className="flex items-center gap-2 text-sm text-success">
            <CheckCircle2 className="h-4 w-4" />
            <span>{t('update.alreadyUpToDate')}</span>
          </div>
        )}

        <Separator />

        {/* Release notes toggle */}
        <div>
          <button
            onClick={() => setShowNotes(!showNotes)}
            className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors"
          >
            {showNotes ? <ChevronUp className="h-3 w-3" /> : <ChevronDown className="h-3 w-3" />}
            {t('update.releaseNotes')}
          </button>
          {showNotes && (
            <div className="mt-2 text-xs text-muted-foreground">
              {changelogLoading ? (
                <div className="flex items-center gap-2">
                  <Loader2 className="h-3 w-3 animate-spin" />
                  {t('update.loadingNotes')}
                </div>
              ) : changelog ? (
                <div className="space-y-2">
                  <div className="flex items-center gap-2 text-foreground font-medium">
                    <span>{changelog.tag_name}</span>
                    <span className="text-muted-foreground">{formatDate(changelog.published_at)}</span>
                  </div>
                  <pre className="whitespace-pre-wrap font-sans bg-muted/50 rounded-lg p-3 max-h-64 overflow-y-auto">
                    {changelog.body}
                  </pre>
                </div>
              ) : null}
            </div>
          )}
        </div>

        {/* Published date */}
        {check.published_at && (
          <p className="text-xs text-muted-foreground">
            {t('update.publishedAt', { date: formatDate(check.published_at) })}
          </p>
        )}
      </CardContent>
    </Card>
  )
}
