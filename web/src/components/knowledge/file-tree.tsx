/**
 * FileTree — Phase 3 redesign (RFC: docs/designs/2026-07-12-knowledge-filetree-design.md)
 *
 * Replaces the previous implementation that used a module-level
 * `expandedDirs` Set + `forceUpdate` hack and lazy-loaded each subdirectory.
 *
 * - Pure prop-driven: receives `KnowledgeTreeNode[]` (one recursive fetch).
 * - Expansion state in `useKnowledgeStore.expandedPaths` (Zustand + persist).
 * - ARIA: `role="treeitem"` + `aria-expanded/level/selected` + `tabIndex`.
 * - Uses `sidebar-accent` tokens (the tree lives in the sidebar).
 * - `depth * 16 + 8` px indentation, unified with workspace tree.
 * - Drag-and-drop reparenting with circular-move guard + flicker-free drag-leave.
 * - Keyboard navigation (Phase 5): ArrowLeft/Right, Enter, Space, F2.
 * - Inline filter (§8.6): flat list of matching files across the subtree.
 * - InlineRenameInput with C3/C4/C7 fixes.
 */
import {
  Bot,
  ChevronRight,
  Diamond,
  File,
  Folder,
  FolderOpen,
  Search as SearchIcon,
  Sparkles,
} from 'lucide-react'
import { useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import {
  countFilesRecursive,
  fileTint,
  flattenTree,
  indentStyle,
} from '@/lib/tree-utils'
import { cn } from '@/lib/utils'
import { useKnowledgeStore } from '@/stores/knowledge'
import type { KnowledgeTreeNode } from '@/types/knowledge'

interface FileTreeProps {
  nodes: KnowledgeTreeNode[]
  currentPath: string | null
  onFileSelect: (path: string) => void
  onRename: (oldPath: string, newName: string) => void
  onContextMenu: (node: KnowledgeTreeNode, x: number, y: number) => void
  onMove?: (from: string, to: string) => void
}

/** S3: detect circular move (folder → itself or one of its descendants). */
function isCircularMove(fromDir: string, toDir: string): boolean {
  if (fromDir === toDir) return true
  return toDir.startsWith(fromDir + '/')
}

function sortNodes(nodes: KnowledgeTreeNode[]): KnowledgeTreeNode[] {
  return [...nodes].sort((a, b) =>
    Number(b.is_dir) - Number(a.is_dir) || a.name.localeCompare(b.name),
  )
}

export function FileTree({
  nodes,
  currentPath,
  onFileSelect,
  onRename,
  onContextMenu,
  onMove,
}: FileTreeProps) {
  const { t } = useTranslation()
  const [filter, setFilter] = useState('')
  const sorted = useMemo(() => sortNodes(nodes), [nodes])

  // §8.6 — Inline filter: client-side case-insensitive match across the
  // whole subtree. When the filter is active the tree is replaced by a
  // flat list of matching files with their full paths.
  const filtered = useMemo(() => {
    const q = filter.trim().toLowerCase()
    if (!q) return null
    return flattenTree(sorted)
      .filter((n) => !n.is_dir && n.path.toLowerCase().includes(q))
      .slice(0, 50)
  }, [filter, sorted])

  if (sorted.length === 0) {
    return (
      <div className="flex flex-col items-center gap-2 px-4 py-8 text-center">
        <File className="h-8 w-8 text-muted-foreground/40" aria-hidden />
        <p className="text-xs text-muted-foreground">{t('knowledge.emptyTree')}</p>
        <p className="text-2xs text-muted-foreground/70">
          {t('knowledge.emptyTreeHint')}
        </p>
      </div>
    )
  }

  return (
    <>
      <div className="sticky top-0 z-10 flex items-center gap-1 border-b border-sidebar-border bg-sidebar/80 px-2 py-1 backdrop-blur">
        <SearchIcon className="h-3 w-3 text-muted-foreground" aria-hidden />
        <input
          value={filter}
          onChange={(e) => setFilter(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Escape') setFilter('')
          }}
          placeholder={t('knowledge.treeFilterPlaceholder')}
          className="flex-1 bg-transparent text-2xs outline-none placeholder:text-muted-foreground/70"
        />
        {filter && (
          <button
            type="button"
            onClick={() => setFilter('')}
            aria-label={t('knowledge.treeFilterClear')}
            className="text-2xs text-muted-foreground hover:text-foreground"
          >
            ×
          </button>
        )}
      </div>
      {filtered ? (
        <ul className="space-y-0.5 pt-1" aria-label={t('knowledge.treeFilterResults')}>
          {filtered.length === 0 ? (
            <li className="px-3 py-2 text-2xs text-muted-foreground">
              {t('knowledge.treeFilterNoMatch')}
            </li>
          ) : (
            filtered.map((node) => (
              <FileTreeFlatRow
                key={node.path}
                node={node}
                currentPath={currentPath}
                onFileSelect={onFileSelect}
                onRename={onRename}
                onContextMenu={onContextMenu}
                onMove={onMove}
              />
            ))
          )}
        </ul>
      ) : (
        <ul role="tree" aria-label="Knowledge files" className="space-y-0.5 pt-1">
          {sorted.map((node) => (
            <FileTreeNode
              key={node.path}
              node={node}
              depth={0}
              currentPath={currentPath}
              onFileSelect={onFileSelect}
              onRename={onRename}
              onContextMenu={onContextMenu}
              onMove={onMove}
            />
          ))}
        </ul>
      )}
    </>
  )
}

interface FileTreeNodeProps {
  node: KnowledgeTreeNode
  depth: number
  currentPath: string | null
  onFileSelect: (path: string) => void
  onRename: (oldPath: string, newName: string) => void
  onContextMenu: (node: KnowledgeTreeNode, x: number, y: number) => void
  onMove?: (from: string, to: string) => void
}

function FileTreeNode({
  node,
  depth,
  currentPath,
  onFileSelect,
  onRename,
  onContextMenu,
  onMove,
}: FileTreeNodeProps) {
  const { expandedPaths, toggleExpand, focusedPath, setFocus, recentlyCreatedPath } = useKnowledgeStore()
  const isExpanded = expandedPaths.includes(node.path)
  const isActive = currentPath === node.path
  const isFocused = focusedPath === node.path
  const shouldBlink = recentlyCreatedPath === node.path
  const [renaming, setRenaming] = useState(false)
  const [dropTarget, setDropTarget] = useState(false)
  const dragCounter = useRef(0) // S3: prevents dragLeave flicker
  const fileCount = useMemo(() => countFilesRecursive(node), [node])

  const handleClick = () => {
    if (node.is_dir) toggleExpand(node.path)
    else onFileSelect(node.path)
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (renaming) return
    if (e.metaKey || e.ctrlKey || e.altKey) return
    if (e.key === 'ArrowRight') {
      e.preventDefault()
      if (node.is_dir && !isExpanded) {
        toggleExpand(node.path)
        setFocus(node.path)
      }
    } else if (e.key === 'ArrowLeft') {
      e.preventDefault()
      if (node.is_dir && isExpanded) {
        toggleExpand(node.path)
        setFocus(node.path)
      }
    } else if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault()
      setFocus(node.path)
      if (node.is_dir) toggleExpand(node.path)
      else onFileSelect(node.path)
    } else if (e.key === 'F2') {
      e.preventDefault()
      if (!node.is_dir) setRenaming(true)
    }
  }

  const handleDragStart = (e: React.DragEvent) => {
    if (node.is_dir || !onMove) return
    e.dataTransfer.setData('text/knowledge-path', node.path)
    e.dataTransfer.effectAllowed = 'move'
  }

  const handleDragOver = (e: React.DragEvent) => {
    if (!node.is_dir || !onMove) return
    e.preventDefault()
    e.dataTransfer.dropEffect = 'move'
  }

  const handleDragEnter = () => {
    if (!node.is_dir || !onMove) return
    dragCounter.current++
    setDropTarget(true)
  }

  const handleDragLeave = () => {
    if (!node.is_dir || !onMove) return
    dragCounter.current--
    if (dragCounter.current <= 0) {
      dragCounter.current = 0
      setDropTarget(false)
    }
  }

  const handleDrop = (e: React.DragEvent) => {
    if (!node.is_dir || !onMove) return
    e.preventDefault()
    dragCounter.current = 0
    setDropTarget(false)
    const from = e.dataTransfer.getData('text/knowledge-path')
    if (!from) return
    const filename = from.split('/').pop() ?? ''
    const target = `${node.path}/${filename}`
    if (target === from) return
    if (isCircularMove(from, target) || isCircularMove(from, node.path)) {
      return
    }
    onMove(from, target)
  }

  const handleContextMenu = (e: React.MouseEvent) => {
    e.preventDefault()
    onContextMenu(node, e.clientX, e.clientY)
  }

  const cancelRename = () => setRenaming(false)
  const submitRename = (newName: string) => {
    setRenaming(false)
    onRename(node.path, newName)
  }

  if (node.is_dir) {
    return (
      <li
        role="treeitem"
        aria-expanded={isExpanded}
        aria-level={depth + 1}
        aria-selected={isActive}
        aria-label={`${node.name}, ${fileCount} ${fileCount === 1 ? 'file' : 'files'}`}
        tabIndex={isFocused ? 0 : -1}
        onKeyDown={handleKeyDown}
      >
        <div
          className={cn(
            'group relative flex items-center gap-2 py-1.5 rounded-lg text-xs w-full text-left select-none transition-all',
            isActive
              ? 'bg-sidebar-accent text-sidebar-accent-foreground font-medium'
              : 'text-sidebar-foreground/70 hover:bg-sidebar-accent/50 hover:text-sidebar-foreground',
            dropTarget && 'ring-1 ring-primary/30 bg-primary/10',
          )}
          style={indentStyle(depth)}
          onClick={handleClick}
          onContextMenu={handleContextMenu}
          onDragOver={handleDragOver}
          onDragEnter={handleDragEnter}
          onDragLeave={handleDragLeave}
          onDrop={handleDrop}
        >
          {isActive && (
            <span
              aria-hidden
              className="absolute left-0 top-1 bottom-1 w-0.5 rounded-r bg-primary"
            />
          )}
          <ChevronRight
            className={cn(
              'h-3 w-3 shrink-0 transition-transform',
              isExpanded && 'rotate-90',
            )}
          />
          {isExpanded ? (
            <FolderOpen className="h-4 w-4 shrink-0 text-muted-foreground" />
          ) : (
            <Folder className="h-4 w-4 shrink-0 text-muted-foreground" />
          )}
          <span className="truncate flex-1">{node.display_name || node.name}</span>
          {fileCount > 0 && (
            <span className="ml-auto text-2xs text-muted-foreground/60 shrink-0">
              {fileCount}
            </span>
          )}
        </div>
        {isExpanded && node.children.length > 0 && (
          <ul role="group" className="space-y-0.5">
            {sortNodes(node.children).map((child) => (
              <FileTreeNode
                key={child.path}
                node={child}
                depth={depth + 1}
                currentPath={currentPath}
                onFileSelect={onFileSelect}
                onRename={onRename}
                onContextMenu={onContextMenu}
                onMove={onMove}
              />
            ))}
          </ul>
        )}
      </li>
    )
  }

  return (
    <li
      role="treeitem"
      aria-level={depth + 1}
      aria-selected={isActive}
      aria-label={node.display_name || node.name}
      tabIndex={isFocused ? 0 : -1}
      onKeyDown={handleKeyDown}
    >
      <div
        draggable={Boolean(onMove)}
        onDragStart={handleDragStart}
        className={cn(
            'group relative flex items-center gap-2 py-1.5 rounded-lg text-xs w-full text-left select-none transition-all',
            isActive
              ? 'bg-sidebar-accent text-sidebar-accent-foreground font-medium'
              : 'text-sidebar-foreground/70 hover:bg-sidebar-accent/50 hover:text-sidebar-foreground',
            shouldBlink && 'animate-file-blink',
        )}
        style={indentStyle(depth)}
        onClick={handleClick}
        onContextMenu={handleContextMenu}
      >
        {isActive && (
          <span
            aria-hidden
            className="absolute left-0 top-1 bottom-1 w-0.5 rounded-r bg-primary"
          />
        )}
        <span className="w-3 shrink-0" aria-hidden />
        <File
          className={cn(
            'h-4 w-4 shrink-0',
            fileTint(node.name),
            !node.has_content && 'opacity-30',
          )}
        />
        {renaming ? (
          <InlineRenameInput
            currentName={node.display_name || node.name}
            onSubmit={submitRename}
            onCancel={cancelRename}
          />
        ) : (
          <span className="truncate flex-1">{node.display_name || node.name}</span>
        )}
        {node.oxios_quality && !renaming && <QualityBadge quality={node.oxios_quality} />}
      </div>
    </li>
  )
}

// Flat row used by the inline filter — renders one matched file path with
// its breadcrumb prefix. Behavior consistent with regular file nodes.
function FileTreeFlatRow({
  node,
  currentPath,
  onFileSelect,
  onRename,
  onContextMenu,
  onMove,
}: Omit<FileTreeNodeProps, 'depth'>) {
  const isActive = currentPath === node.path
  const [renaming, setRenaming] = useState(false)
  return (
    <li role="treeitem" aria-level={1} aria-selected={isActive} aria-label={node.path}>
      <div
        draggable={Boolean(onMove)}
        onDragStart={(e) => {
          if (node.is_dir || !onMove) return
          e.dataTransfer.setData('text/knowledge-path', node.path)
          e.dataTransfer.effectAllowed = 'move'
        }}
        className={cn(
          'group relative flex items-center gap-2 rounded-lg py-1 px-2 text-xs w-full text-left select-none transition-all',
          isActive
            ? 'bg-sidebar-accent text-sidebar-accent-foreground font-medium'
            : 'text-sidebar-foreground/70 hover:bg-sidebar-accent/50 hover:text-sidebar-foreground',
        )}
        onClick={() => onFileSelect(node.path)}
        onContextMenu={(e) => {
          e.preventDefault()
          onContextMenu(node, e.clientX, e.clientY)
        }}
      >
        {isActive && (
          <span
            aria-hidden
            className="absolute left-0 top-1 bottom-1 w-0.5 rounded-r bg-primary"
          />
        )}
        <File
          className={cn(
            'h-3.5 w-3.5 shrink-0',
            fileTint(node.name),
            !node.has_content && 'opacity-30',
          )}
        />
        {renaming ? (
          <InlineRenameInput
            currentName={node.display_name || node.name}
            onSubmit={(newName) => {
              setRenaming(false)
              onRename(node.path, newName)
            }}
            onCancel={() => setRenaming(false)}
          />
        ) : (
          <span className="truncate flex-1" title={node.path}>
            {node.path}
          </span>
        )}
      </div>
    </li>
  )
}

function QualityBadge({ quality }: { quality: 'raw' | 'curated' | 'refined' }) {
  const { t } = useTranslation()
  const labelKey = {
    raw: 'knowledge.qualityRaw',
    curated: 'knowledge.qualityCurated',
    refined: 'knowledge.qualityRefined',
  }[quality]
  return (
    <span
      title={t(labelKey)}
      className={cn(
        'ml-auto shrink-0 flex items-center gap-0.5 text-2xs px-1 rounded',
        quality === 'raw' && 'text-muted-foreground bg-muted',
        quality === 'curated' && 'text-success bg-success-muted',
        quality === 'refined' && 'text-info bg-info-muted',
      )}
    >
      {quality === 'raw' && <Bot className="h-3.5 w-3.5" aria-hidden />}
      {quality === 'curated' && <Sparkles className="h-3.5 w-3.5" aria-hidden />}
      {quality === 'refined' && <Diamond className="h-3.5 w-3.5" aria-hidden />}
    </span>
  )
}

function InlineRenameInput({
  currentName,
  onSubmit,
  onCancel,
}: {
  currentName: string
  onSubmit: (newName: string) => void
  onCancel: () => void
}) {
  const [value, setValue] = useState(currentName)
  const cancelledRef = useRef(false)

  const handleSubmit = () => {
    const trimmed = value.trim()
    if (!trimmed) {
      onCancel()
      return
    }
    const newName = trimmed.endsWith('.md') ? trimmed : `${trimmed}.md`
    onSubmit(newName)
  }

  return (
    <input
      autoFocus
      // @ts-expect-error — non-standard selectAll is widely supported
      selectAll
      className="flex-1 rounded bg-sidebar/80 px-1 text-xs outline-none ring-1 ring-ring"
      value={value}
      onChange={(e) => setValue(e.target.value)}
      onKeyDown={(e) => {
        if (e.key === 'Enter') {
          e.preventDefault()
          cancelledRef.current = false
          handleSubmit()
        } else if (e.key === 'Escape') {
          cancelledRef.current = true
          onCancel()
        }
      }}
      onBlur={() => {
        if (!cancelledRef.current) handleSubmit()
      }}
    />
  )
}
