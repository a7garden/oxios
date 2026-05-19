import { createFileRoute } from '@tanstack/react-router'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { api } from '@/lib/api-client'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Badge } from '@/components/ui/badge'
import { LoadingCards } from '@/components/shared/loading'
import { EmptyState } from '@/components/shared/empty-state'
import { GitBranch, RefreshCw, Tag, RotateCcw, ShieldCheck } from 'lucide-react'
import { useState } from 'react'
import type { GitCommit } from '@/types'

export const Route = createFileRoute('/git')({ component: GitPage })

function GitPage() {
  const queryClient = useQueryClient()
  const [tagName, setTagName] = useState('')
  const [restoreHash, setRestoreHash] = useState('')

  const { data: commits, isLoading, refetch, isFetching } = useQuery({
    queryKey: ['git-log'],
    queryFn: () => api.get<GitCommit[]>('/api/git/log'),
  })

  const { data: tags } = useQuery({
    queryKey: ['git-tags'],
    queryFn: () => api.get<string[]>('/api/git/tags'),
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
    mutationFn: () => api.post('/api/git/verify'),
  })

  if (isLoading) return <LoadingCards count={4} />

  const commitList = commits ?? []
  const tagList = tags ?? []

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Git</h1>
          <p className="text-muted-foreground">In-process version control</p>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" size="sm" onClick={() => refetch()} disabled={isFetching}>
            <RefreshCw className={`h-4 w-4 mr-1 ${isFetching ? 'animate-spin' : ''}`} /> Refresh
          </Button>
          <Button variant="outline" size="sm" onClick={() => verifyMutation.mutate()} disabled={verifyMutation.isPending}>
            <ShieldCheck className="h-4 w-4 mr-1" /> Verify
          </Button>
        </div>
      </div>

      {/* Tags */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Tag className="h-4 w-4" /> Tags
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="flex gap-2 mb-3">
            <Input
              value={tagName}
              onChange={(e) => setTagName(e.target.value)}
              placeholder="Tag name..."
              className="max-w-xs"
            />
            <Button
              size="sm"
              onClick={() => tagMutation.mutate(tagName)}
              disabled={!tagName.trim() || tagMutation.isPending}
            >
              Create Tag
            </Button>
          </div>
          {tagList.length > 0 ? (
            <div className="flex gap-2 flex-wrap">
              {tagList.map((t) => (
                <Badge key={t} variant="outline">{t}</Badge>
              ))}
            </div>
          ) : (
            <p className="text-sm text-muted-foreground">No tags.</p>
          )}
        </CardContent>
      </Card>

      {/* Restore */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <RotateCcw className="h-4 w-4" /> Restore
          </CardTitle>
        </CardHeader>
        <CardContent>
          <div className="flex gap-2">
            <Input
              value={restoreHash}
              onChange={(e) => setRestoreHash(e.target.value)}
              placeholder="Commit hash to restore..."
              className="max-w-xs font-mono"
            />
            <Button
              size="sm"
              variant="destructive"
              onClick={() => restoreMutation.mutate(restoreHash)}
              disabled={!restoreHash.trim() || restoreMutation.isPending}
            >
              Restore
            </Button>
          </div>
        </CardContent>
      </Card>

      {/* Log */}
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <GitBranch className="h-4 w-4" /> Commit Log
          </CardTitle>
        </CardHeader>
        <CardContent>
          {commitList.length === 0 ? (
            <EmptyState
              icon={<GitBranch className="h-8 w-8" />}
              title="No commits"
              description="Commit history will appear here."
              className="py-6"
            />
          ) : (
            <div className="space-y-2">
              {commitList.map((commit) => (
                <div key={commit.hash} className="flex items-start gap-3 rounded-lg border p-3">
                  <code className="text-xs bg-muted px-1.5 py-0.5 rounded shrink-0">
                    {commit.hash.slice(0, 7)}
                  </code>
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
    </div>
  )
}
