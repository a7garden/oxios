import { useCallback } from 'react'
import { FilePlus, FolderPlus, Trash2 } from 'lucide-react'
import { useKnowledgeStore } from '@/stores/knowledge'
import { useKnowledgeTree, useWriteFile, useDeleteFile } from '@/hooks/use-knowledge'
import { FileTree } from './file-tree'
import { ResizeHandle } from './resize-handle'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'

export function KnowledgeSidebar() {
  const { sidebarWidth, setSidebarWidth, currentFilePath, mode, openFile, openChat } = useKnowledgeStore()
  const { data: entries, isLoading, refetch } = useKnowledgeTree()
  const writeFile = useWriteFile()
  const deleteFile = useDeleteFile()

  const handleNewFile = useCallback(async () => {
    const name = 'New file.md'
    // Simple unique name — server will handle full validation
    await writeFile.mutateAsync({ path: name, content: `# New file\n\n` })
    openFile(name)
    refetch()
  }, [writeFile, openFile, refetch])

  const handleNewFolder = useCallback(async () => {
    const name = prompt('Enter folder name:', 'New Folder')
    if (!name?.trim()) return
    // Create a placeholder file in the folder to ensure it exists
    await writeFile.mutateAsync({ path: `${name.trim()}/.keep`, content: '' })
    refetch()
  }, [writeFile, refetch])

  return (
    <div
      className="flex flex-col h-full border-r bg-muted/30 shrink-0"
      style={{ width: sidebarWidth }}
    >
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-2 border-b">
        <span className="text-sm font-medium text-muted-foreground">Notes</span>
        <div className="flex gap-1">
          <Button
            variant="ghost"
            size="icon"
            className="h-7 w-7"
            onClick={handleNewFile}
            title="New file (⌘N)"
          >
            <FilePlus className="h-4 w-4" />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            className="h-7 w-7"
            onClick={handleNewFolder}
            title="New folder (⌘⇧N)"
          >
            <FolderPlus className="h-4 w-4" />
          </Button>
          {currentFilePath && (
            <Button
              variant="ghost"
              size="icon"
              className="h-7 w-7 text-destructive"
              onClick={async () => {
                if (confirm(`Delete ${currentFilePath}?`)) {
                  await deleteFile.mutateAsync(currentFilePath)
                }
              }}
              title="Delete current file (⌘D)"
            >
              <Trash2 className="h-4 w-4" />
            </Button>
          )}
        </div>
      </div>

      {/* Chat button */}
      <button
        type="button"
        onClick={() => openChat()}
        className={cn(
          'flex items-center gap-2 px-3 py-2 text-sm w-full text-left hover:bg-accent/50 transition-colors border-b',
          mode === 'chat' && 'bg-accent font-medium',
        )}
      >
        💬 Chat
      </button>

      {/* File tree */}
      <div className="flex-1 overflow-y-auto p-1">
        {isLoading ? (
          <div className="p-3 text-sm text-muted-foreground">Loading...</div>
        ) : entries ? (
          <FileTree entries={entries} onFileSelect={openFile} currentPath={currentFilePath} />
        ) : null}
      </div>

      {/* Resize handle */}
      <ResizeHandle width={sidebarWidth} onResize={setSidebarWidth} />
    </div>
  )
}
