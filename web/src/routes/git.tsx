import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { GitBranch, RotateCcw, ShieldCheck, Tag } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { EmptyState } from '@/components/shared/empty-state'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { RefreshButton } from '@/components/shared/refresh-button'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { api } from '@/lib/api-client'
import type { GitCommit } from '@/types'

export const Route = createFileRoute('/git')({ component: GitPage })

function GitPage() {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const [tagName, setTagName] = useState('')
  const [restoreHash, setRestoreHash] = useState('')
  const [restoreConfirm, setRestoreConfirm] = useState(false)

  const {
    data: commits,
    isLoading,
    isError,
    refetch,
    isFetching,
  } = useQuery({
    queryKey: ['git-log'],
    queryFn: async () => {
      const res = await api.get<{ entries: GitCommit[] }>('/api/git/log')
      return Array.isArray(res?.entries) ? res.entries : []
    },
    refetchInterval: 15000,
  })

  const { data: tags } = useQuery({
    queryKey: ['git-tags'],
    queryFn: async () => {
      const res = await api.get<{ tags: string[] }>('/api/git/tags')
      return Array.isArray(res?.tags) ? res.tags : []
    },
    refetchInterval: 15000,
  })

  const tagMutation = useMutation({
    mutationFn: (tag: string) => api.post('/api/git/tags', { tag }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['git-tags'] })
      setTagName('')
    },
  })

  const restoreMutation = useMutation({
    mutationFn: (hash: string) => api.post('/api/git/restore', { hash }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['git-log'] })
      setRestoreHash('')
    },
  })

  const verifyMutation = useMutation({
    mutationFn: () => api.post<{ valid?: boolean; message?: string }>('/api/git/verify'),
    onSuccess: (res) => {
      toast.success(res?.message ?? t('git.verifySuccess', 'Git integrity verified'))
    },
    onError: (err) => {
      toast.error(err instanceof Error ? err.message : t('git.verifyFailed', 'Verification failed'))
    },
  })

  if (isLoading) return <LoadingCards count={4} />
  if (isError) return <ErrorState onRetry={() => refetch()} />

  const commitList = Array.isArray(commits) ? commits : []
  const tagList = Array.isArray(tags) ? tags : []

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">{t('git.title')}</h1>
          <p className="text-muted-foreground">{t('git.subtitle')}</p>
        </div>
        <div className="flex gap-2">
          <RefreshButton onClick={() => refetch()} isFetching={isFetching} />
          <Button
            variant="outline"
            size="sm"
            onClick={() => verifyMutation.mutate()}
            disabled={verifyMutation.isPending}
          >
            <ShieldCheck className="h-4 w-4 mr-1" /> {t('git.verify')}
          </Button>
        </div>
      </div>

      {/* Tags */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Tag className="h-4 w-4" /> {t('git.tags')}
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="flex gap-2 mb-3">
            <Input
              value={tagName}
              onChange={(e) => setTagName(e.target.value)}
              placeholder={t('git.tagNamePlaceholder')}
              className="max-w-xs"
            />
            <Button
              size="sm"
              onClick={() => tagMutation.mutate(tagName)}
              disabled={!tagName.trim() || tagMutation.isPending}
            >
              {t('git.createTag')}
            </Button>
          </div>
          {tagList.length > 0 ? (
            <div className="flex gap-2 flex-wrap">
              {tagList.map((t) => (
                <Badge key={t} variant="outline">
                  {t}
                </Badge>
              ))}
            </div>
          ) : (
            <p className="text-sm text-muted-foreground">{t('git.noTags')}</p>
          )}
        </CardContent>
      </Card>

      {/* Restore */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <RotateCcw className="h-4 w-4" /> {t('git.restore')}
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="flex gap-2">
            <Input
              value={restoreHash}
              onChange={(e) => setRestoreHash(e.target.value)}
              placeholder={t('git.restorePlaceholder')}
              className="max-w-xs font-mono"
            />
            <Button
              size="sm"
              variant="destructive"
              onClick={() => setRestoreConfirm(true)}
              disabled={!restoreHash.trim() || restoreMutation.isPending}
            >
              {t('git.restore')}
            </Button>
          </div>
        </CardContent>
      </Card>

      {/* Log */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <GitBranch className="h-4 w-4" /> {t('git.commitLog')}
          </CardTitle>
        </CardHeader>
        <CardContent>
          {commitList.length === 0 ? (
            <EmptyState
              icon={<GitBranch className="h-8 w-8" />}
              title={t('git.noCommits')}
              description={t('git.noCommitsDescription')}
              className="py-6"
            />
          ) : (
            <div className="space-y-2">
              {commitList.map((commit) => (
                <div key={commit.hash} className="flex items-start gap-3 rounded-lg border p-3">
                  <button
                    type="button"
                    onClick={() => setRestoreHash(commit.hash)}
                    title={t('git.useHashForRestore')}
                    className="text-xs bg-muted px-1.5 py-0.5 rounded shrink-0 font-mono cursor-pointer hover:bg-accent hover:text-accent-foreground"
                  >
                    {commit.hash.slice(0, 7)}
                  </button>
                  <div className="flex-1 min-w-0">
                    <p className="text-sm truncate">{commit.message}</p>
                    <p className="text-xs text-muted-foreground">
                      {commit.author} • {new Date(commit.timestamp).toLocaleString()}
                    </p>
                  </div>
                </div>
              ))}
            </div>
          )}
        </CardContent>
      </Card>
      <Dialog open={restoreConfirm} onOpenChange={setRestoreConfirm}>
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <DialogTitle>{t('git.restoreConfirmTitle')}</DialogTitle>
            <DialogDescription>
              {t('git.restoreConfirmDesc', { hash: restoreHash.slice(0, 7) })}
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button
              variant="ghost"
              size="sm"
              onClick={() => setRestoreConfirm(false)}
              disabled={restoreMutation.isPending}
            >
              {t('common.cancel')}
            </Button>
            <Button
              variant="destructive"
              size="sm"
              disabled={!restoreHash.trim() || restoreMutation.isPending}
              onClick={() => {
                restoreMutation.mutate(restoreHash)
                setRestoreConfirm(false)
              }}
            >
              {t('git.restore')}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  )
}
