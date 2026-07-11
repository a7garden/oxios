import { useKnowledgeFile, useWriteFile } from '@/hooks/use-knowledge'
import { MarkdownEditor } from './markdown-editor'

interface SplitEditorProps {
  filePath: string
}

export function SplitEditor({ filePath }: SplitEditorProps) {
  const { data: content } = useKnowledgeFile(filePath)
  const writeFile = useWriteFile()

  return (
    <div className="flex flex-col flex-1 md:w-1/2 md:flex-none min-w-0 min-h-0 border-t md:border-l md:border-t-0">
      <div className="flex items-center gap-2 px-4 py-2 text-sm font-medium border-b bg-muted/30">
        <span className="truncate flex-1">{filePath.split('/').pop()?.replace(/\.md$/, '')}</span>
      </div>
      <div className="flex-1 overflow-hidden">
        <MarkdownEditor
          key={filePath}
          filePath={filePath}
          initialContent={content ?? ''}
          onSave={async (content) => {
            await writeFile.mutateAsync({ path: filePath, content })
          }}
        />
      </div>
    </div>
  )
}
