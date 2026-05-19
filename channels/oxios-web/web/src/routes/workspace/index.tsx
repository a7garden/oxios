import { createFileRoute } from '@tanstack/react-router'
import { useQuery } from '@tanstack/react-query'
import { api } from '@/lib/api-client'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { LoadingCards } from '@/components/shared/loading'
import { EmptyState } from '@/components/shared/empty-state'
import { FolderOpen, RefreshCw, ChevronRight, ChevronDown, File, Folder } from 'lucide-react'
import { useState } from 'react'
import type { FileNode } from '@/types'

export const Route = createFileRoute('/workspace/')({ component: WorkspacePage })

function WorkspacePage() {
  const [expandedPaths, setExpandedPaths] = useState<Set<string>>(new Set())

  const { data: root, isLoading, refetch, isFetching } = useQuery({
    queryKey: ['workspace'],
    queryFn: () => api.get<FileNode>('/api/workspace'),
  })

  const toggleExpand = async (node: FileNode) => {
    if (node.type !== 'directory') return
    setExpandedPaths((prev) => {
      const next = new Set(prev)
      if (next.has(node.path)) {
        next.delete(node.path)
      } else {
        next.add(node.path)
      }
      return next
    })
  }

  const { data: children, refetch: refetchChildren } = useQuery({
    queryKey: ['workspace-children', [...expandedPaths]],
    queryFn: async () => {
      const result: Record<string, FileNode[]> = {}
      for (const path of expandedPaths) {
        try {
          const dir = await api.get<FileNode[]>(`/api/workspace/${encodeURIComponent(path)}`)
          result[path] = dir
        } catch {
          result[path] = []
        }
      }
      return result
    },
    enabled: expandedPaths.size > 0,
  })

  if (isLoading) return <LoadingCards count={4} />

  const renderNode = (node: FileNode, depth: number = 0) => {
    const isExpanded = expandedPaths.has(node.path)
    const nodeChildren = isExpanded ? children?.[node.path] ?? node.children ?? [] : node.children ?? []

    return (
      <div key={node.path}>
        <div
          className="flex items-center gap-2 py-1.5 px-2 hover:bg-muted/50 rounded cursor-pointer text-sm"
          style={{ paddingLeft: `${depth * 16 + 8}px` }}
          onClick={() => node.type === 'directory' && toggleExpand(node)}
        >
          {node.type === 'directory' ? (
            <>
              {isExpanded ? <ChevronDown className="h-4 w-4 shrink-0" /> : <ChevronRight className="h-4 w-4 shrink-0" />}
              <Folder className="h-4 w-4 text-amber-500 shrink-0" />
            </>
          ) : (
            <>
              <span className="w-4" />
              <File className="h-4 w-4 text-muted-foreground shrink-0" />
            </>
          )}
          <span className="truncate">{node.name}</span>
          {node.size != null && (
            <span className="ml-auto text-xs text-muted-foreground">
              {node.size > 1024 ? `${(node.size / 1024).toFixed(1)}KB` : `${node.size}B`}
            </span>
          )}
          {node.modified && (
            <span className="text-xs text-muted-foreground shrink-0">
              {new Date(node.modified).toLocaleDateString()}
            </span>
          )}
        </div>
        {isExpanded && nodeChildren.map((child) => renderNode(child, depth + 1))}
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
          {!root ? (
            <EmptyState
              icon={<FolderOpen className="h-8 w-8" />}
              title="No workspace"
              description="Workspace files will appear here."
              className="py-6"
            />
          ) : (
            <div className="space-y-0">
              {renderNode(root)}
              {root.children?.length === 0 && (
                <p className="text-sm text-muted-foreground p-4">Empty workspace.</p>
              )}
            </div>
          )}
        </CardContent>
      </Card>
    </div>
  )
}
