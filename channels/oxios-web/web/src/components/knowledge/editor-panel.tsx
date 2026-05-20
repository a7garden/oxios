import { useKnowledgeStore } from '@/stores/knowledge'
import { useKnowledgeFile, useWriteFile } from '@/hooks/use-knowledge'
import { EditorToolbar } from './editor-toolbar'
import { MarkdownEditor } from './markdown-editor'
import { SplitEditor } from './split-editor'

export function EditorPanel() {
  const { currentFilePath, splitEditorOpen, splitFilePath } = useKnowledgeStore()
  const { data: content, isLoading } = useKnowledgeFile(currentFilePath)
  const writeFile = useWriteFile()

  return (
    <div className="flex flex-1 min-w-0">
      {/* Main editor */}
      <div className="flex flex-col flex-1 min-w-0">
        <EditorToolbar />
        <div className="flex-1 overflow-hidden">
          {isLoading ? (
            <div className="flex items-center justify-center h-full text-muted-foreground">
              Loading...
            </div>
          ) : currentFilePath ? (
            <MarkdownEditor
              key={currentFilePath}
              filePath={currentFilePath}
              initialContent={content ?? ''}
              onSave={(content) => writeFile.mutate({ path: currentFilePath, content })}
            />
          ) : (
            <div className="flex items-center justify-center h-full text-muted-foreground">
              Select a file or open chat
            </div>
          )}
        </div>
      </div>

      {/* Split editor */}
      {splitEditorOpen && splitFilePath && (
        <SplitEditor filePath={splitFilePath} />
      )}
    </div>
  )
}
