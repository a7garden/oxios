import { useKnowledgeFile, useWriteFile } from '@/hooks/use-knowledge'
import { MarkdownEditor } from './markdown-editor'

interface SplitEditorProps {
  filePath: string
}

export function SplitEditor({ filePath }: SplitEditorProps) {
  const { data: content } = useKnowledgeFile(filePath)
  const writeFile = useWriteFile()

  return (
    <div className="w-1/2 min-w-[200px] border-l flex flex-col">
      <div className="px-4 py-2 text-sm font-medium border-b bg-muted/30 truncate">
        {filePath.split('/').pop()?.replace(/\.md$/, '')}
      </div>
      <div className="flex-1 overflow-hidden">
        <MarkdownEditor
          key={filePath}
          filePath={filePath}
          initialContent={content ?? ''}
          onSave={(content) => writeFile.mutate({ path: filePath, content })}
        />
      </div>
    </div>
  )
}
