import { useRouterState } from '@tanstack/react-router'
import { ArrowRightLeft } from 'lucide-react'
import { useCallback, useEffect, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import {
  useDeleteFile,
  useKnowledgeFile,
  useKnowledgeTree,
  useWriteFile,
} from '@/hooks/use-knowledge'
import { cn } from '@/lib/utils'
import { useKnowledgeStore } from '@/stores/knowledge'
import type { KnowledgeTreeEntry } from '@/types/knowledge'

export function MoveModal() {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')
  const [focusedIndex, setFocusedIndex] = useState(0)
  const inputRef = useRef<HTMLInputElement>(null)

  const currentFilePath = useKnowledgeStore((s) => s.currentFilePath)
  const openFile = useKnowledgeStore((s) => s.openFile)
  const { data: treeEntries } = useKnowledgeTree()
  const { data: currentContent } = useKnowledgeFile(currentFilePath)
  const writeFile = useWriteFile()
  const deleteFile = useDeleteFile()

  // Extract directories from tree
  const allDirs = extractDirectories(treeEntries)

  // Filter by query
  const filteredDirs = query.trim()
    ? allDirs.filter((d) => d.toLowerCase().includes(query.toLowerCase()))
    : allDirs

  // Global ⌘M listener (M5: pathname via ref)
  const router = useRouterState()
  const pathnameRef = useRef(router.location.pathname)
  pathnameRef.current = router.location.pathname

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (!pathnameRef.current.startsWith('/knowledge')) return
      if ((e.metaKey || e.ctrlKey) && e.key === 'm') {
        // Don't open if no file is selected
        if (!currentFilePath) return
        e.preventDefault()
        e.stopPropagation()
        setOpen(true)
        setQuery('')
        setFocusedIndex(0)
      }
    }
    window.addEventListener('keydown', handler, true)
    return () => window.removeEventListener('keydown', handler, true)
  }, [currentFilePath])

  // Focus input on open
  useEffect(() => {
    if (open) inputRef.current?.focus()
  }, [open])

  // Reset focusedIndex when filtered list changes
  useEffect(() => {
    setFocusedIndex(0)
  }, [])

  const close = useCallback(() => setOpen(false), [])

  const handleMove = useCallback(
    async (targetDir: string) => {
      if (!currentFilePath || currentContent == null) return

      const filename = currentFilePath.split('/').pop()!
      const newPath = targetDir === '/' ? filename : `${targetDir}/${filename}`

      if (newPath === currentFilePath) {
        close()
        return
      }

      // Write to new location, then delete old
      await writeFile.mutateAsync({ path: newPath, content: currentContent })
      await deleteFile.mutateAsync(currentFilePath)
      openFile(newPath)
      close()
    },
    [currentFilePath, currentContent, writeFile, deleteFile, openFile, close],
  )

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault()
        close()
      } else if (e.key === 'ArrowDown') {
        e.preventDefault()
        setFocusedIndex((i) => (i + 1) % Math.max(filteredDirs.length, 1))
      } else if (e.key === 'ArrowUp') {
        e.preventDefault()
        setFocusedIndex(
          (i) => (i - 1 + Math.max(filteredDirs.length, 1)) % Math.max(filteredDirs.length, 1),
        )
      } else if (e.key === 'Enter') {
        e.preventDefault()
        if (filteredDirs[focusedIndex] !== undefined) {
          handleMove(filteredDirs[focusedIndex])
        }
      }
    },
    [filteredDirs, focusedIndex, close, handleMove],
  )

  if (!open) return null

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center pt-[30vh]">
      {/* Backdrop — click outside to close */}
      <div className="fixed inset-0 bg-black/50" onClick={close} />

      <div className="relative w-full max-w-md bg-background border rounded-lg shadow-lg overflow-hidden">
        {/* Header with search input */}
        <div className="flex items-center gap-2 p-3 border-b">
          <ArrowRightLeft className="h-4 w-4 text-muted-foreground shrink-0" />
          <input
            ref={inputRef}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={t('knowledge.moveToFolder')}
            className="flex-1 bg-transparent text-sm outline-none placeholder:text-muted-foreground"
          />
          <kbd className="text-xs text-muted-foreground border rounded px-1.5 py-0.5">ESC</kbd>
        </div>

        {/* Directory list */}
        <ul className="max-h-80 overflow-y-auto">
          {filteredDirs.length > 0 ? (
            filteredDirs.map((dir, i) => (
              <li
                key={dir}
                aria-selected={i === focusedIndex}
                className={cn(
                  'px-4 py-2.5 text-sm cursor-pointer transition-colors',
                  i === focusedIndex ? 'bg-accent' : 'hover:bg-accent/50',
                )}
                onClick={() => handleMove(dir)}
                onMouseEnter={() => setFocusedIndex(i)}
              >
                {dir === '/' ? '/' : `${dir}/`}
              </li>
            ))
          ) : (
            <li className="px-4 py-6 text-sm text-muted-foreground text-center">
              {t('knowledge.noMatchingFolders')}
            </li>
          )}
        </ul>
      </div>
    </div>
  )
}

/**
 * Extract a flat directory list from root-level tree entries.
 * Returns `['/', ...dirNames]` sorted with underscore-prefixed dirs last.
 */
function extractDirectories(entries?: KnowledgeTreeEntry[]): string[] {
  if (!entries) return ['/']
  const dirs: string[] = ['/']
  for (const entry of entries) {
    if (entry.is_dir && !entry.name.startsWith('.') && entry.name !== 'media') {
      dirs.push(entry.name)
    }
  }
  // Sort: underscore dirs last, then alphabetical
  dirs.sort((a, b) => {
    const aUnderscore = a.startsWith('_') ? 1 : 0
    const bUnderscore = b.startsWith('_') ? 1 : 0
    if (aUnderscore !== bUnderscore) return aUnderscore - bUnderscore
    return a.localeCompare(b)
  })
  return dirs
}
