/**
 * QuickOpen — Cmd+P file finder modal.
 *
 * Fetches the project file list once on open, then fuzzy-filters as the
 * user types. Arrow keys navigate, Enter opens the file in the editor,
 * Escape closes. Matches VS Code / Zed / Sublime behaviour.
 */
/** Simple fuzzy subsequence matcher (same algorithm as fuzzysearch). */
function fuzzy(query: string, target: string): boolean {
  if (!query) return true
  let qi = 0
  for (let ti = 0; ti < target.length && qi < query.length; ti++) {
    if (target[ti] === query[qi]) qi++
  }
  return qi === query.length
}

import { File, Search, X } from 'lucide-react'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { codeApi } from '@/lib/code-api'
import { cn } from '@/lib/utils'
import { useCodeSessionStore } from '@/stores/code/code-session'

interface QuickOpenProps {
  projectPath: string
  onClose: () => void
}

export function QuickOpen({ projectPath, onClose }: QuickOpenProps) {
  const [query, setQuery] = useState('')
  const [files, setFiles] = useState<string[]>([])
  const [loading, setLoading] = useState(true)
  const [selected, setSelected] = useState(0)
  const inputRef = useRef<HTMLInputElement | null>(null)
  const listRef = useRef<HTMLDivElement | null>(null)

  const addTab = useCodeSessionStore((s) => s.addTab)

  // Fetch file list on mount.
  useEffect(() => {
    let cancelled = false
    codeApi
      .listFiles(projectPath)
      .then((list) => {
        if (!cancelled) {
          // Sort shorter paths first — they're usually more relevant.
          list.sort((a, b) => a.length - b.length)
          setFiles(list)
          setLoading(false)
        }
      })
      .catch(() => {
        if (!cancelled) setLoading(false)
      })
    return () => {
      cancelled = true
    }
  }, [projectPath])

  // Focus input on mount.
  useEffect(() => {
    inputRef.current?.focus()
  }, [])

  // Filter files by fuzzy query.
  const filtered = useMemo(() => {
    if (!query) return files.slice(0, 50)
    const q = query.toLowerCase()
    return files
      .filter((f) => {
        const rel = f.startsWith(projectPath) ? f.slice(projectPath.length + 1) : f
        return fuzzy(q, rel.toLowerCase())
      })
      .slice(0, 50)
  }, [files, query, projectPath])

  // Reset selection when filter changes.
  useEffect(() => {
    setSelected(0)
  }, [filtered])

  // Scroll selected item into view.
  useEffect(() => {
    const el = listRef.current?.children[selected] as HTMLElement | undefined
    el?.scrollIntoView({ block: 'nearest' })
  }, [selected])

  const openFile = useCallback(
    async (path: string) => {
      try {
        const content = await codeApi.readFile(path)
        addTab({
          id: crypto.randomUUID(),
          path,
          name: path.split('/').pop() || path,
          language: content.language,
          isDirty: false,
          isPreview: false,
          content: content.content,
        })
      } catch {
        /* ignore — file may be binary or unreadable */
      }
      onClose()
    },
    [addTab, onClose],
  )

  const onKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'ArrowDown') {
        e.preventDefault()
        setSelected((s) => Math.min(s + 1, filtered.length - 1))
      } else if (e.key === 'ArrowUp') {
        e.preventDefault()
        setSelected((s) => Math.max(s - 1, 0))
      } else if (e.key === 'Enter') {
        e.preventDefault()
        const file = filtered[selected]
        if (file) openFile(file)
      } else if (e.key === 'Escape') {
        e.preventDefault()
        onClose()
      }
    },
    [filtered, selected, openFile, onClose],
  )

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center bg-black/40 pt-[15vh]"
      onClick={onClose}
    >
      <div
        className="w-full max-w-xl overflow-hidden rounded-lg border border-line bg-surface shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Search input */}
        <div className="flex items-center gap-2 border-b border-line px-3 py-2.5">
          <Search className="size-4 text-muted-foreground" />
          <input
            ref={inputRef}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={onKeyDown}
            placeholder="Search files by name…"
            className="flex-1 bg-transparent text-sm placeholder:text-muted-foreground focus:outline-none"
          />
          <button
            type="button"
            onClick={onClose}
            className="rounded p-0.5 text-muted-foreground hover:text-foreground"
          >
            <X className="size-4" />
          </button>
        </div>

        {/* Results */}
        <div ref={listRef} className="max-h-[50vh] overflow-y-auto">
          {loading && (
            <div className="px-3 py-4 text-center text-sm text-muted-foreground">
              Loading files…
            </div>
          )}
          {!loading && filtered.length === 0 && (
            <div className="px-3 py-4 text-center text-sm text-muted-foreground">
              No files found.
            </div>
          )}
          {!loading &&
            filtered.map((file, i) => {
              const rel = file.startsWith(projectPath) ? file.slice(projectPath.length + 1) : file
              const parts = rel.split('/')
              const name = parts.pop() ?? rel
              const dir = parts.join('/')
              return (
                <button
                  type="button"
                  key={file}
                  className={cn(
                    'flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm transition-colors',
                    i === selected ? 'bg-primary/10 text-foreground' : 'hover:bg-surface-sunken',
                  )}
                  onMouseEnter={() => setSelected(i)}
                  onClick={() => openFile(file)}
                >
                  <File className="size-3.5 shrink-0 text-muted-foreground" />
                  <span className="truncate font-medium">{name}</span>
                  {dir && <span className="truncate text-xs text-muted-foreground">{dir}</span>}
                </button>
              )
            })}
        </div>
      </div>
    </div>
  )
}
