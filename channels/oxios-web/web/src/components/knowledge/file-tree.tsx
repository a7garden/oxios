import { useState } from 'react'
import { ChevronRight, File, Folder, FolderOpen } from 'lucide-react'
import { useKnowledgeTree } from '@/hooks/use-knowledge'
import type { KnowledgeTreeEntry } from '@/types/knowledge'
import { cn } from '@/lib/utils'

interface FileTreeProps {
  entries: KnowledgeTreeEntry[]
  onFileSelect: (path: string) => void
  currentPath: string | null
  parentPath?: string
}

export function FileTree({ entries, onFileSelect, currentPath, parentPath = '' }: FileTreeProps) {
  return (
    <ul className="space-y-0.5">
      {entries.map((entry) => (
        <FileTreeItem
          key={entry.name}
          entry={entry}
          parentPath={parentPath}
          onFileSelect={onFileSelect}
          currentPath={currentPath}
        />
      ))}
    </ul>
  )
}

interface FileTreeItemProps {
  entry: KnowledgeTreeEntry
  parentPath: string
  onFileSelect: (path: string) => void
  currentPath: string | null
}

function FileTreeItem({ entry, parentPath, onFileSelect, currentPath }: FileTreeItemProps) {
  const [expanded, setExpanded] = useState(false)
  const fullPath = parentPath ? `${parentPath}/${entry.name}` : entry.name
  const isActive = currentPath === fullPath

  if (entry.is_dir) {
    // Don't show hidden/system dirs
    if (entry.name.startsWith('.') || entry.name === 'media') return null
    return (
      <li>
        <button
          type="button"
          onClick={() => setExpanded(!expanded)}
          className="flex items-center gap-1.5 w-full px-2 py-1 text-sm rounded hover:bg-accent/50 transition-colors text-left"
        >
          <ChevronRight className={cn('h-3 w-3 shrink-0 transition-transform', expanded && 'rotate-90')} />
          {expanded ? (
            <FolderOpen className="h-4 w-4 shrink-0 text-muted-foreground" />
          ) : (
            <Folder className="h-4 w-4 shrink-0 text-muted-foreground" />
          )}
          <span className="truncate">{entry.name}</span>
        </button>
        {expanded && <SubDirectory dir={fullPath} onFileSelect={onFileSelect} currentPath={currentPath} />}
      </li>
    )
  }

  // File
  // Don't show config or hidden files
  if (entry.name === 'config.json' || entry.name.startsWith('.')) return null

  return (
    <li>
      <button
        type="button"
        onClick={() => onFileSelect(fullPath)}
        className={cn(
          'flex items-center gap-1.5 w-full px-2 py-1 text-sm rounded transition-colors text-left',
          isActive ? 'bg-accent font-medium' : 'hover:bg-accent/50',
        )}
      >
        <span className="w-4 shrink-0" /> {/* indent spacer */}
        <File className="h-4 w-4 shrink-0 text-muted-foreground" />
        <span className="truncate">{entry.name.replace(/\.md$/, '')}</span>
      </button>
    </li>
  )
}

function SubDirectory({ dir, onFileSelect, currentPath }: { dir: string; onFileSelect: (path: string) => void; currentPath: string | null }) {
  const { data: entries, isLoading } = useKnowledgeTree(dir)
  if (isLoading) return <div className="pl-4 text-xs text-muted-foreground">...</div>
  if (!entries || entries.length === 0) return null
  return (
    <div className="pl-3">
      <FileTree entries={entries} onFileSelect={onFileSelect} currentPath={currentPath} parentPath={dir} />
    </div>
  )
}
