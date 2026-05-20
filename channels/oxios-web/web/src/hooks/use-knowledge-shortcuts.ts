import { useEffect, useRef } from 'react'
import { useRouterState } from '@tanstack/react-router'
import { useKnowledgeStore } from '@/stores/knowledge'
import { useWriteFile, useDeleteFile } from '@/hooks/use-knowledge'

/**
 * Register global keyboard shortcuts for the Knowledge UI.
 * Only active when the current route is within /knowledge.
 * Uses router state (via ref) instead of window.location for accuracy (M5).
 *
 * Shortcuts:
 * ⌘N        → New file
 * ⌘⇧N       → New folder
 * ⌘D        → Delete current file (editor mode)
 * ⌘Enter    → Open chat
 * ⌘⇧Enter   → Toggle chat overlay
 * ⌘~ / ⌘§   → Toggle sidebar
 * ⌘W        → Close split editor
 * Escape    → Close split / deselect all
 */
export function useKnowledgeShortcuts() {
  const store = useKnowledgeStore()
  const writeFile = useWriteFile()
  const deleteFile = useDeleteFile()

  // Keep a ref to the latest pathname so the event handler is never stale (M5)
  const router = useRouterState()
  const pathnameRef = useRef(router.location.pathname)
  pathnameRef.current = router.location.pathname

  useEffect(() => {
    const handler = async (e: KeyboardEvent) => {
      // Only activate when on a knowledge route
      if (!pathnameRef.current.startsWith('/knowledge')) return

      const isMeta = e.metaKey || e.ctrlKey

      // ⌘N — new file
      if (isMeta && e.key === 'n' && !e.shiftKey) {
        e.preventDefault()
        e.stopPropagation()
        try {
          await writeFile.mutateAsync({ path: 'New file.md', content: '# New file\n\n' })
          store.openFile('New file.md')
        } catch { /* ignore */ }
        return
      }

      // ⌘⇧N — new folder
      if (isMeta && e.key === 'N' && e.shiftKey) {
        e.preventDefault()
        e.stopPropagation()
        const name = prompt('Enter folder name:', 'New Folder')
        if (name?.trim()) {
          try {
            await writeFile.mutateAsync({ path: `${name.trim()}/.keep`, content: '' })
          } catch { /* ignore */ }
        }
        return
      }

      // ⌘D — delete current file
      if (isMeta && e.key === 'd') {
        e.preventDefault()
        e.stopPropagation()
        if (store.mode === 'editor' && store.currentFilePath) {
          if (confirm(`Delete ${store.currentFilePath}?`)) {
            try {
              await deleteFile.mutateAsync(store.currentFilePath)
            } catch { /* ignore */ }
          }
        }
        return
      }

      // ⌘Enter — open chat
      if (isMeta && e.key === 'Enter' && !e.shiftKey) {
        e.preventDefault()
        e.stopPropagation()
        store.openChat()
        return
      }

      // ⌘⇧Enter — toggle chat overlay
      if (isMeta && e.key === 'Enter' && e.shiftKey) {
        e.preventDefault()
        e.stopPropagation()
        store.openChat()
        return
      }

      // ⌘~ or ⌘§ — toggle sidebar
      if (isMeta && (e.key === '~' || e.key === '§')) {
        e.preventDefault()
        e.stopPropagation()
        store.toggleSidebar()
        return
      }

      // ⌘W — close split editor
      if (isMeta && e.key === 'w') {
        e.preventDefault()
        e.stopPropagation()
        if (store.splitEditorOpen) {
          store.closeSplit()
        }
        return
      }

      // Escape — close split or deselect
      if (e.key === 'Escape') {
        if (store.splitEditorOpen) {
          store.closeSplit()
        }
        return
      }
    }

    window.addEventListener('keydown', handler, true)
    return () => window.removeEventListener('keydown', handler, true)
  }, [store, writeFile, deleteFile])
}
