import { useQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { ChevronDown, ChevronRight, File, Folder, FolderOpen, RefreshCw } from 'lucide-react'
import { useState } from 'react'
import { EmptyState } from '@/components/shared/empty-state'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { api } from '@/lib/api-client'
import type { TreeEntry } from '@/types'

export const Route = createFileRoute('/workspace/')({ component: WorkspacePage })

function WorkspacePage() {
  const [expandedPaths, setExpandedPaths] = useState<Set<string>>(new Set())

  const {
    data: entries,
    isLoading,
    isError,
    refetch,
    isFetching,
  } = useQuery({
    queryKey: ['workspace'],
    queryFn: async () => {
      const res = await api.get<TreeEntry[]>('/api/workspace/tree')
      return Array.isArray(res) ? res : []
    },
    refetchInterval: 15000,
  })

  const toggleExpand = (name: string) => {
    setExpandedPaths((prev) => {
      const next = new Set(prev)
      if (next.has(name)) next.delete(name)
      else next.add(name)
      return next
    })
  }

  // Fetch children for expanded directories
  const expandedArr = [...expandedPaths]
  const { data: childrenMap } = useQuery({
    queryKey: ['workspace-children', expandedArr],
    queryFn: async () => {
      const result: Record<string, TreeEntry[]> = {}
      for (const dir of expandedArr) {
        try {
          const res = await api.get<TreeEntry[]>(`/api/workspace/tree?dir=${encodeURIComponent(dir)}`)
          result[dir] = Array.isArray(res) ? res : []
        } catch {
          result[dir] = []
        }
      }
      return result
    },
    enabled: expandedArr.length > 0,
  })

  if (isLoading) return <LoadingCards count={4} />
  if (isError) return <ErrorState onRetry={() => refetch()} />

  const renderEntry = (entry: TreeEntry, depth: number = 0) => {
    const isExpanded = expandedPaths.has(entry.name)

    return (
      <div key={entry.name}>
        <div
          role="treeitem"
          tabIndex={0}
          className="flex items-center gap-2 py-1.5 px-2 hover:bg-muted/50 rounded cursor-pointer text-sm"
          style={{ paddingLeft: `${depth * 16 + 8}px` }}
          onClick={() => entry.is_dir && toggleExpand(entry.name)}
          onKeyDown={(e) => {
            if (e.key === 'Enter' || e.key === ' ') {
              e.preventDefault()
              entry.is_dir && toggleExpand(entry.name)
            }
          }}
        >
          {entry.is_dir ? (
            <>
              {isExpanded ? (
                <ChevronDown className="h-4 w-4 shrink-0" />
              ) : (
                <ChevronRight className="h-4 w-4 shrink-0" />
              )}
              <Folder className="h-4 w-4 text-amber-500 shrink-0" />
            </>
          ) : (
            <>
              <span className="w-4" />
              <File className="h-4 w-4 text-muted-foreground shrink-0" />
            </>
          )}
          <span className="truncate">{entry.name}</span>
          {!entry.is_dir && entry.size > 0 && (
            <span className="ml-auto text-xs text-muted-foreground">
              {entry.size > 1024 ? `${(entry.size / 1024).toFixed(1)}KB` : `${entry.size}B`}
            </span>
          )}
        </div>
        {isExpanded && entry.is_dir && (childrenMap?.[entry.name] ?? []).map((child) => renderEntry(child, depth + 1))}
      </div>
    )
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Workspace</h1>
          <p className="text-muted-foreground">File browser</p>
        </div>
        <Button variant="outline" size="sm" onClick={() => refetch()} disabled={isFetching}>
          <RefreshCw className={`h-4 w-4 mr-1 ${isFetching ? 'animate-spin' : ''}`} /> Refresh
        </Button>
      </div>

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <FolderOpen className="h-4 w-4" /> Files
          </CardTitle>
        </CardHeader>
        <CardContent>
          {!entries || entries.length === 0 ? (
            <EmptyState
              icon={<FolderOpen className="h-8 w-8" />}
              title="No workspace"
              description="Workspace files will appear here."
              className="py-6"
            />
          ) : (
            <div className="space-y-0">
              {entries.map((entry) => renderEntry(entry))}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  )
}
