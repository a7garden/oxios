import { Bot, ChevronRight, File, Folder, FolderOpen, Gem, Sparkles } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useKnowledgeTree } from '@/hooks/use-knowledge'
import { cn } from '@/lib/utils'
import type { KnowledgeTreeEntry } from '@/types/knowledge'

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
          expandedDirs={expandedDirs}
          toggleDir={toggleDir}
        />
      ))}
    </ul>
  )
}

// Global expanded dirs state shared across the tree
const expandedDirs = new Set<string>()

function toggleDir(path: string) {
  if (expandedDirs.has(path)) {
    expandedDirs.delete(path)
  } else {
    expandedDirs.add(path)
  }
}

interface FileTreeItemProps {
  entry: KnowledgeTreeEntry
  parentPath: string
  onFileSelect: (path: string) => void
  currentPath: string | null
  expandedDirs: Set<string>
  toggleDir: (path: string) => void
}

function FileTreeItem({
  entry,
  parentPath,
  onFileSelect,
  currentPath,
  expandedDirs,
  toggleDir,
}: FileTreeItemProps) {
  const fullPath = parentPath ? `${parentPath}/${entry.name}` : entry.name
  const isActive = currentPath === fullPath
  const expanded = expandedDirs.has(fullPath)

  // Force re-render when expanded changes
  const [, forceUpdate] = useState(0)
  const toggle = () => {
    toggleDir(fullPath)
    forceUpdate((n) => n + 1)
  }

  if (entry.is_dir) {
    // Don't show hidden/system dirs
    if (entry.name.startsWith('.') || entry.name === 'media') return null
    return (
      <li>
        <button
          type="button"
          onClick={toggle}
          className="flex items-center gap-1.5 w-full px-2.5 py-1.5 text-sm rounded hover:bg-accent/50 transition-colors text-left"
        >
          <ChevronRight
            className={cn('h-3 w-3 shrink-0 transition-transform', expanded && 'rotate-90')}
          />
          {expanded ? (
            <FolderOpen className="h-4 w-4 shrink-0 text-muted-foreground" />
          ) : (
            <Folder className="h-4 w-4 shrink-0 text-muted-foreground" />
          )}
          <span className="truncate">{entry.name}</span>
        </button>
        {expanded && (
          <SubDirectory dir={fullPath} onFileSelect={onFileSelect} currentPath={currentPath} />
        )}
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
          'flex items-center gap-1.5 w-full px-2.5 py-1.5 text-sm rounded transition-colors text-left',
          isActive ? 'bg-accent font-medium' : 'hover:bg-accent/50',
        )}
      >
        <span className="w-4 shrink-0" /> {/* indent spacer */}
        <File className="h-4 w-4 shrink-0 text-muted-foreground" />
        <span className="truncate">{entry.name.replace(/\.md$/, '')}</span>
        {entry.oxios_quality && (
          <span
            className={cn(
              'ml-auto shrink-0 flex items-center gap-0.5 text-2xs',
              entry.oxios_quality === 'raw' && 'text-muted-foreground',
              entry.oxios_quality === 'curated' && 'text-green-600',
              entry.oxios_quality === 'refined' && 'text-blue-600',
            )}
          >
            {entry.oxios_quality === 'raw' && <Bot className="h-2.5 w-2.5" />}
            {entry.oxios_quality === 'curated' && <Sparkles className="h-2.5 w-2.5" />}
            {entry.oxios_quality === 'refined' && <Gem className="h-2.5 w-2.5" />}
          </span>
        )}
      </button>
    </li>
  )
}

function SubDirectory({
  dir,
  onFileSelect,
  currentPath,
}: {
  dir: string
  onFileSelect: (path: string) => void
  currentPath: string | null
}) {
  const { data: entries, isLoading } = useKnowledgeTree(dir)
  const { t } = useTranslation()
  if (isLoading)
    return <div className="pl-4 text-xs text-muted-foreground">{t('knowledge.loading')}</div>
  if (!entries || entries.length === 0) return null
  return (
    <div className="pl-4">
      <FileTree
        entries={entries}
        onFileSelect={onFileSelect}
        currentPath={currentPath}
        parentPath={dir}
      />
    </div>
  )
}
