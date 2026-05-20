import { ArrowLeft, ArrowRight, Columns2, X } from 'lucide-react'
import { useKnowledgeStore } from '@/stores/knowledge'
import { Button } from '@/components/ui/button'

export function EditorToolbar() {
  const {
    currentFilePath,
    history,
    historyIndex,
    goBack,
    goForward,
    infoPanelOpen,
    toggleInfoPanel,
    splitEditorOpen,
    closeSplit,
  } = useKnowledgeStore()

  const canGoBack = historyIndex > 0
  const canGoForward = historyIndex < history.length - 1
  const fileName = currentFilePath?.split('/').pop()?.replace(/\.md$/, '') ?? ''

  return (
    <div className="flex items-center gap-1 px-2 py-1 border-b bg-muted/30 min-h-[36px]">
      <Button
        variant="ghost"
        size="icon"
        className="h-7 w-7"
        onClick={() => goBack()}
        disabled={!canGoBack}
        title="Go back"
      >
        <ArrowLeft className="h-4 w-4" />
      </Button>
      <Button
        variant="ghost"
        size="icon"
        className="h-7 w-7"
        onClick={() => goForward()}
        disabled={!canGoForward}
        title="Go forward"
      >
        <ArrowRight className="h-4 w-4" />
      </Button>

      <span className="text-sm font-medium truncate mx-2">
        {fileName || 'Knowledge'}
      </span>

      <div className="flex-1" />

      {/* Split editor toggle */}
      {splitEditorOpen && (
        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7"
          onClick={closeSplit}
          title="Close split (⌘W)"
        >
          <X className="h-4 w-4" />
        </Button>
      )}
    </div>
  )
}
