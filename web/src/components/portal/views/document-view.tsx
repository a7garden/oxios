// DocumentView — portal view for editing a KnowledgeBase markdown file.
//
// LobeHub analogue: features/Portal/Document (full editor suite). Oxios version
// reuses the existing CodeMirror 6 MarkdownEditor + KB hooks (useKnowledgeFile /
// useWriteFile). No new editor is built — this is a thin portal adapter.

import { Loader2 } from 'lucide-react'
import { useCallback } from 'react'
import { MarkdownEditor } from '@/components/knowledge/markdown-editor'
import { useKnowledgeFile, useWriteFile } from '@/hooks/use-knowledge'
import type { PortalView } from '@/stores/portal'

interface DocumentViewProps {
  view: Extract<PortalView, { type: 'document' }>
}

export function DocumentView({ view }: DocumentViewProps) {
  const { path } = view
  const { data: content, isLoading } = useKnowledgeFile(path)
  const writeFile = useWriteFile()

  const handleSave = useCallback(
    async (newContent: string) => {
      await writeFile.mutateAsync({ path, content: newContent })
    },
    [path, writeFile],
  )

  if (isLoading) {
    return (
      <div className="flex h-full items-center justify-center text-muted-foreground">
        <Loader2 className="size-5 animate-spin" />
      </div>
    )
  }

  return (
    <div className="h-full overflow-hidden">
      <MarkdownEditor
        filePath={path}
        initialContent={content ?? ''}
        onSave={handleSave}
        className="h-full"
      />
    </div>
  )
}
