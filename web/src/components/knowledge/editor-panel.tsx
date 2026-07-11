import { FileText } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useKnowledgeFile, useWriteFile } from '@/hooks/use-knowledge'
import { useKnowledgeStore } from '@/stores/knowledge'
import { EditorStatusBar, type EditorStats } from './editor-status-bar'
import { EditorToolbar } from './editor-toolbar'
import { MarkdownEditor } from './markdown-editor'
import { SplitEditor } from './split-editor'

export function EditorPanel() {
  const { t } = useTranslation()
  const { currentFilePath, splitEditorOpen, splitFilePath } = useKnowledgeStore()
  const { data: content, isLoading } = useKnowledgeFile(currentFilePath)
  const writeFile = useWriteFile()
  const [stats, setStats] = useState<EditorStats | null>(null)

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
                key={currentFilePath}
                filePath={currentFilePath}
                initialContent={content ?? ''}
                onSave={async (content) => {
                  await writeFile.mutateAsync({ path: currentFilePath, content })
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
