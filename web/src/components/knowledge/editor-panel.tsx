import { FileText } from 'lucide-react'
import { useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import {
  useKnowledgeFile,
  useKnowledgeRecursiveTree,
  useMoveFile,
  useWriteFile,
} from '@/hooks/use-knowledge'
import { desiredRenamePath, extractH1 } from '@/lib/note-rename'
import { flattenTree } from '@/lib/tree-utils'
import { useKnowledgeStore } from '@/stores/knowledge'
import { type EditorStats, EditorStatusBar } from './editor-status-bar'
import { EditorToolbar } from './editor-toolbar'
import { MarkdownEditor } from './markdown-editor'
import { SplitEditor } from './split-editor'

export function EditorPanel() {
  const { t } = useTranslation()
  const { currentFilePath, editorSessionId, splitEditorOpen, splitFilePath, renameCurrent } =
    useKnowledgeStore()
  const { data: content, isLoading } = useKnowledgeFile(currentFilePath)
  const { data: tree } = useKnowledgeRecursiveTree()
  const writeFile = useWriteFile()
  const moveFile = useMoveFile()
  const [stats, setStats] = useState<EditorStats | null>(null)

  // Snapshot of every known file path — used to refuse a rename whose
  // target would clobber a different existing note. Rebuilt only when the
  // recursive tree refetches (after every write/move).
  const knownPaths = useMemo(() => {
    const set = new Set<string>()
    if (tree) for (const n of flattenTree(tree)) set.add(n.path)
    return set
  }, [tree])

  return (
    <div className="flex flex-col md:flex-row flex-1 min-w-0">
      {/* Main editor */}
      <div className="flex flex-col flex-1 min-w-0 min-h-0">
        <EditorToolbar />
        <div className="flex-1 overflow-hidden">
          {isLoading ? (
            <div className="flex items-center justify-center h-full text-muted-foreground">
              {t('knowledge.loading')}
            </div>
          ) : currentFilePath ? (
            <div className="mx-auto max-w-4xl h-full px-4 sm:px-8 lg:px-16">
              <MarkdownEditor
                key={editorSessionId}
                filePath={currentFilePath}
                initialContent={content ?? ''}
                onSave={async (nextContent) => {
                  await writeFile.mutateAsync({ path: currentFilePath, content: nextContent })

                  // H1 → rename. The editor's heading enforcer keeps line 1
                  // as `# <title>`; if that title implies a different
                  // filename, move the note so the sidebar tree, search,
                  // and links all follow the visible title.
                  const target = desiredRenamePath(currentFilePath, extractH1(nextContent))
                  if (!target) return
                  // Refuse to clobber a different existing note.
                  if (knownPaths.has(target)) {
                    toast.error(t('knowledge.renameCollision'))
                    return
                  }
                  try {
                    await moveFile.mutateAsync({ from: currentFilePath, to: target })
                    // Swap the open path in place — the editor stays
                    // mounted (keyed on editorSessionId, which is NOT
                    // bumped here) so cursor + undo history survive.
                    renameCurrent(target)
                  } catch {
                    toast.error(t('knowledge.saveFailed'))
                  }
                }}
                onStatsChange={setStats}
              />
            </div>
          ) : (
            <div className="flex flex-col items-center justify-center h-full text-muted-foreground gap-3">
              <FileText className="h-10 w-10 opacity-20" />
              <div className="text-center">
                <p className="font-medium">{t('knowledge.noFileSelected')}</p>
                <p className="text-sm mt-1">{t('knowledge.noFileSelectedHint')}</p>
              </div>
            </div>
          )}
        </div>
        {currentFilePath && <EditorStatusBar stats={stats} />}
      </div>

      {/* Split editor */}
      {splitEditorOpen && splitFilePath && <SplitEditor filePath={splitFilePath} />}
    </div>
  )
}
