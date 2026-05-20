import { useState, useEffect, useRef, useCallback } from 'react'
import { Search, Folder, FileText } from 'lucide-react'
import { useKnowledgeStore } from '@/stores/knowledge'
import { useKnowledgeSearch, useKnowledgeTree } from '@/hooks/use-knowledge'
import type { KnowledgeSearchHit, KnowledgeTreeEntry } from '@/types/knowledge'
import { cn } from '@/lib/utils'

interface SearchModalProps {
  /** When opened externally (e.g. from chat message action) */
  forceOpen?: boolean
  /** When set, modal acts as a file picker for moving a chat message */
  selectedMessageText?: string | null
  /** Called in moveMessage mode when a file is selected */
  onMoveToFile?: (path: string) => void
  /** Called in moveMessage mode when a directory is selected */
  onMoveToDir?: (dir: string) => void
  /** Called when the modal closes */
  onClose?: () => void
}

/** A single item that can appear in the results list (search hit, file, or directory). */
interface ResultItem {
  path: string
  name: string
  isDir: boolean
}

const MAX_RECENT_FILES = 15

export function SearchModal({
  forceOpen,
  selectedMessageText,
  onMoveToFile,
  onMoveToDir,
  onClose,
}: SearchModalProps) {
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')
  const [focusedIndex, setFocusedIndex] = useState(0)
  const inputRef = useRef<HTMLInputElement>(null)
  const listRef = useRef<HTMLUListElement>(null)

  const openFile = useKnowledgeStore((s) => s.openFile)
  const searchMutation = useKnowledgeSearch()
  const { data: treeEntries } = useKnowledgeTree()

  const isMoveMode = selectedMessageText != null
  const [searchResults, setSearchResults] = useState<KnowledgeSearchHit[]>([])

  // ── Build the display list ──────────────────────────────────
  const recentFiles: ResultItem[] = (treeEntries ?? [])
    .filter((e: KnowledgeTreeEntry) => !e.is_dir)
    .slice(0, MAX_RECENT_FILES)
    .map((e: KnowledgeTreeEntry) => ({ path: e.name, name: e.name, isDir: false }))

  const treeDirs: ResultItem[] = (treeEntries ?? [])
    .filter((e: KnowledgeTreeEntry) => e.is_dir)
    .map((e: KnowledgeTreeEntry) => ({ path: e.name, name: e.name, isDir: true }))

  const displayItems: ResultItem[] = (() => {
    if (query.trim() && searchResults.length > 0) {
      return searchResults.map((h: KnowledgeSearchHit) => ({
        path: h.path,
        name: h.name,
        isDir: false,
      }))
    }
    // In move mode, show directories first then files
    if (isMoveMode) {
      return [...treeDirs, ...recentFiles]
    }
    // Normal empty state: just show recent files
    return recentFiles
  })()

  // ── Global ⌘K / ⌘P listener (only on knowledge routes) ───────────
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (!window.location.pathname.startsWith('/knowledge')) return
      if ((e.metaKey || e.ctrlKey) && (e.key === 'k' || e.key === 'p')) {
        e.preventDefault()
        e.stopPropagation()
        setOpen(true)
        setQuery('')
        setFocusedIndex(0)
        setSearchResults([])
      }
    }
    window.addEventListener('keydown', handler, true)
    return () => window.removeEventListener('keydown', handler, true)
  }, [])

  // ── Focus input when opening ────────────────────────────────
  useEffect(() => {
    if (open) {
      // Small delay so the DOM mounts before focusing
      const id = requestAnimationFrame(() => inputRef.current?.focus())
      return () => cancelAnimationFrame(id)
    }
  }, [open])

  // ── Respond to forceOpen prop ───────────────────────────────
  useEffect(() => {
    if (forceOpen) {
      setOpen(true)
      setQuery('')
      setFocusedIndex(0)
      setSearchResults([])
    }
  }, [forceOpen])

  // ── Auto-search on query change (debounced) ─────────────────
  useEffect(() => {
    if (!open) return
    if (!query.trim()) {
      setSearchResults([])
      setFocusedIndex(0)
      return
    }
    const timer = setTimeout(() => {
      searchMutation.mutate(
        { query, limit: 20 },
        {
          onSuccess: (data) => {
            setSearchResults(data.results)
            setFocusedIndex(0)
          },
          onError: () => {
            setSearchResults([])
          },
        },
      )
    }, 150)
    return () => clearTimeout(timer)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [query, open])

  // ── Close handler ───────────────────────────────────────────
  const close = useCallback(() => {
    setOpen(false)
    onClose?.()
  }, [onClose])

  // ── Select handler ──────────────────────────────────────────
  const handleSelect = useCallback(
    (item: ResultItem) => {
      if (isMoveMode) {
        if (item.isDir && onMoveToDir) {
          onMoveToDir(item.path)
        } else if (!item.isDir && onMoveToFile) {
          onMoveToFile(item.path)
        }
      } else {
        openFile(item.path)
      }
      close()
    },
    [isMoveMode, onMoveToDir, onMoveToFile, openFile, close],
  )

  // ── Keyboard navigation inside modal ────────────────────────
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      const count = Math.max(displayItems.length, 1)

      if (e.key === 'Escape') {
        e.preventDefault()
        close()
        return
      }

      if (e.key === 'ArrowDown') {
        e.preventDefault()
        setFocusedIndex((i) => (i + 1) % count)
        return
      }

      if (e.key === 'ArrowUp') {
        e.preventDefault()
        setFocusedIndex((i) => (i - 1 + count) % count)
        return
      }

      if (e.key === 'Enter') {
        e.preventDefault()
        const item = displayItems[focusedIndex]
        if (item) {
          handleSelect(item)
        }
        return
      }
    },
    [displayItems, focusedIndex, close, handleSelect],
  )

  // ── Scroll focused item into view ───────────────────────────
  useEffect(() => {
    if (!listRef.current) return
    const focusedEl = listRef.current.children[focusedIndex] as HTMLElement | undefined
    focusedEl?.scrollIntoView({ block: 'nearest' })
  }, [focusedIndex])

  // ── Early return when closed ────────────────────────────────
  if (!open) return null

  const hasQuery = query.trim().length > 0
  const isSearching = searchMutation.isPending && hasQuery

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center pt-[20vh]">
      {/* Backdrop */}
      <div className="fixed inset-0 bg-black/50" onClick={close} />

      {/* Dialog */}
      <div className="relative w-full max-w-lg bg-background border rounded-lg shadow-lg overflow-hidden">
        {/* Search input row */}
        <div className="flex items-center gap-2 px-3 py-2.5 border-b">
          <Search className="h-4 w-4 text-muted-foreground shrink-0" />
          <input
            ref={inputRef}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={
              isMoveMode
                ? 'Search or select a destination...'
                : 'Search files...'
            }
            className="flex-1 bg-transparent text-sm outline-none placeholder:text-muted-foreground"
          />
          <kbd className="text-[10px] text-muted-foreground border rounded px-1.5 py-0.5 font-mono">
            ESC
          </kbd>
        </div>

        {/* Results list */}
        <ul ref={listRef} className="max-h-80 overflow-y-auto p-1">
          {displayItems.length > 0 ? (
            displayItems.map((item, i) => (
              <li
                key={item.path + (item.isDir ? '-dir' : '-file')}
                className={cn(
                  'flex items-center gap-2.5 px-3 py-2 text-sm cursor-pointer rounded-md transition-colors',
                  i === focusedIndex
                    ? 'bg-accent text-accent-foreground'
                    : 'hover:bg-accent/50',
                )}
                onClick={() => handleSelect(item)}
                onMouseEnter={() => setFocusedIndex(i)}
              >
                {item.isDir ? (
                  <Folder className="h-4 w-4 shrink-0 text-muted-foreground" />
                ) : (
                  <FileText className="h-4 w-4 shrink-0 text-muted-foreground" />
                )}
                <span className="font-medium truncate">
                  {item.name.replace(/\.md$/, '')}
                </span>
                {item.path.includes('/') && (
                  <span className="ml-auto text-xs text-muted-foreground shrink-0">
                    {item.path.replace(/\/[^/]+$/, '')}
                  </span>
                )}
                {item.isDir && (
                  <span className="ml-auto text-xs text-muted-foreground shrink-0">
                    dir
                  </span>
                )}
              </li>
            ))
          ) : hasQuery ? (
            <li className="px-4 py-6 text-sm text-muted-foreground text-center">
              {isSearching ? 'Searching...' : 'No results'}
            </li>
          ) : (
            <li className="px-4 py-6 text-sm text-muted-foreground text-center">
              {isMoveMode
                ? 'Select a file or directory'
                : 'Type to search files'}
            </li>
          )}
        </ul>

        {/* Footer hint */}
        <div className="flex items-center justify-between border-t px-3 py-1.5 text-[10px] text-muted-foreground">
          <span>
            <kbd className="font-mono border rounded px-1">↑↓</kbd> navigate
            {' · '}
            <kbd className="font-mono border rounded px-1">↵</kbd> select
          </span>
          <span>
            <kbd className="font-mono border rounded px-1">⌘K</kbd> to open
          </span>
        </div>
      </div>
    </div>
  )
}
