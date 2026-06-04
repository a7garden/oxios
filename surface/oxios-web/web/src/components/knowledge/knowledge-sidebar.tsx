import { Link } from '@tanstack/react-router'
import { BookOpen, FilePlus, FolderPlus, LayoutDashboard, MessageSquare, PanelLeftClose, Trash2 } from 'lucide-react'
import { useCallback } from 'react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import {
  useDeleteFile,
  useJournalToday,
  useKnowledgeTree,
  useWriteFile,
} from '@/hooks/use-knowledge'
import { cn } from '@/lib/utils'
import { useKnowledgeStore } from '@/stores/knowledge'
import { FileTree } from './file-tree'
import { ResizeHandle } from './resize-handle'

/**
 * Knowledge sidebar — replaces the main AppLayout sidebar when in Knowledge mode.
 * Renders the file tree, chat/journal buttons, and new file/folder actions.
 */
export function KnowledgeSidebar() {
  const { t } = useTranslation()
  const {
    sidebarWidth,
    setSidebarWidth,
    currentFilePath,
    mode,
    openFile,
    openChat,
    toggleSidebar,
  } = useKnowledgeStore()
  const { data: entries, isLoading, refetch } = useKnowledgeTree()
  const writeFile = useWriteFile()
  const deleteFile = useDeleteFile()

  const handleNewFile = useCallback(async () => {
    const name = 'New file.md'
    await writeFile.mutateAsync({ path: name, content: `# New file\n\n` })
    openFile(name)
    refetch()
  }, [writeFile, openFile, refetch])

  const handleNewFolder = useCallback(async () => {
    const name = prompt('Enter folder name:', 'New Folder')
    if (!name?.trim()) return
    await writeFile.mutateAsync({ path: `${name.trim()}/.keep`, content: '' })
    refetch()
  }, [writeFile, refetch])

  const journalToday = useJournalToday()
  const handleOpenJournal = useCallback(() => {
    if (journalToday.data?.path) {
      openFile(journalToday.data.path)
    }
  }, [journalToday.data, openFile])

  return (
    <div
      className="flex flex-col h-full border-r bg-sidebar text-sidebar-foreground shrink-0"
      style={{ width: sidebarWidth }}
    >
      {/* Header — matches main sidebar style */}
      <div className="flex items-center justify-between px-3 h-14 border-b border-sidebar-border">
        <div className="flex items-center gap-2">
          <span className="font-bold text-lg">Notes</span>
        </div>
        <div className="flex items-center gap-0.5">
          {/* Primary actions — always visible */}
          <Button
            variant="ghost"
            size="icon"
            className="h-7 w-7 hover:bg-sidebar-accent/50"
            onClick={handleNewFile}
            title={t('knowledge.newFileShortcut')}
          >
            <FilePlus className="h-4 w-4" />
          </Button>
          {/* Secondary actions — hidden on narrow screens (B4) */}
          <div className="hidden sm:flex items-center gap-0.5">
            <Button
              variant="ghost"
              size="icon"
              className="h-7 w-7 hover:bg-sidebar-accent/50"
              onClick={handleNewFolder}
              title={t('knowledge.newFolderShortcut')}
            >
              <FolderPlus className="h-4 w-4" />
            </Button>
            {currentFilePath && (
              <Button
                variant="ghost"
                size="icon"
                className="h-7 w-7 text-destructive hover:bg-sidebar-accent/50"
                onClick={async () => {
                  if (confirm(`Delete ${currentFilePath}?`)) {
                    await deleteFile.mutateAsync(currentFilePath)
                  }
                }}
                title={t('knowledge.deleteCurrentFile')}
              >
                <Trash2 className="h-4 w-4" />
              </Button>
            )}
          </div>
          {/* Collapse sidebar button */}
          <Button
            variant="ghost"
            size="icon"
            className="h-7 w-7 hover:bg-sidebar-accent/50"
            onClick={toggleSidebar}
            aria-label={t('common.closeSidebar')}
            title={t('knowledge.collapseSidebar')}
          >
            <PanelLeftClose className="h-4 w-4" />
          </Button>
        </div>
      </div>

      {/* Chat button */}
      <button
        type="button"
        onClick={() => openChat()}
        className={cn(
          'flex items-center gap-2.5 px-4 py-2.5 text-sm w-full text-left transition-colors border-b border-sidebar-border',
          mode === 'chat'
            ? 'bg-sidebar-accent text-sidebar-accent-foreground font-medium'
            : 'hover:bg-sidebar-accent/50 text-sidebar-foreground/70',
        )}
      >
        <MessageSquare className="h-4 w-4" />
        Chat
      </button>

      {/* Journal button */}
      <button
        type="button"
        onClick={handleOpenJournal}
        disabled={journalToday.isLoading}
        className="flex items-center gap-2.5 px-4 py-2.5 text-sm w-full text-left hover:bg-sidebar-accent/50 transition-colors border-b border-sidebar-border text-sidebar-foreground/70 disabled:opacity-50"
      >
        <BookOpen className="h-4 w-4" />
        {t('knowledge.toJournal')}
      </button>

      {/* File tree */}
      <div className="flex-1 overflow-y-auto px-2 py-2">
        {isLoading ? (
          <div className="p-3 text-sm text-sidebar-foreground/50">{t('knowledge.loading')}</div>
        ) : entries ? (
          <FileTree entries={entries} onFileSelect={openFile} currentPath={currentFilePath} />
        ) : null}
      </div>

      {/* Resize handle */}
      <ResizeHandle width={sidebarWidth} onResize={setSidebarWidth} />

      {/* Keyboard shortcuts legend */}
      <div className="px-3 py-3 border-t border-sidebar-border text-[11px] text-sidebar-foreground/40 space-y-1">
        <div className="flex items-center gap-2">
          <kbd className="font-mono border rounded px-1.5 py-0.5">⌘K</kbd>
          <span>{t('knowledge.search')}</span>
        </div>
        <div className="flex items-center gap-2">
          <kbd className="font-mono border rounded px-1.5 py-0.5">⌘M</kbd>
          <span>{t('knowledge.moveFile')}</span>
        </div>
        <div className="flex items-center gap-2">
          <kbd className="font-mono border rounded px-1.5 py-0.5">⌘N</kbd>
          <span>{t('knowledge.newFile')}</span>
        </div>
      </div>

      {/* Navigation back to Dashboard */}
      <div className="px-2 py-3 border-t border-sidebar-border">
        <Link
          to="/"
          className="flex items-center gap-2 rounded-lg px-2.5 py-2 text-sm text-sidebar-foreground/70 hover:bg-sidebar-accent/50 hover:text-sidebar-foreground transition-colors w-full"
        >
          <LayoutDashboard className="h-4 w-4 shrink-0" />
          <span>{t('common.dashboard')}</span>
        </Link>
      </div>
    </div>
  )
}
