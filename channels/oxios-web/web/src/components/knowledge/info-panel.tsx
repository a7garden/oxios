import { PanelRightClose } from 'lucide-react'
import { useKnowledgeStore } from '@/stores/knowledge'
import { useKnowledgeBacklinks } from '@/hooks/use-knowledge'
import { Button } from '@/components/ui/button'

export function InfoPanel() {
  const { currentFilePath, toggleInfoPanel, openFile } = useKnowledgeStore()
  const { data: backlinks, isLoading } = useKnowledgeBacklinks(currentFilePath)

  return (
    <div className="w-64 border-l bg-muted/30 flex flex-col shrink-0">
      <div className="flex items-center justify-between px-3 py-2 border-b">
        <span className="text-sm font-medium">Info</span>
        <Button variant="ghost" size="icon" className="h-7 w-7" onClick={toggleInfoPanel}>
          <PanelRightClose className="h-4 w-4" />
        </Button>
      </div>

      {/* Backlinks */}
      <div className="p-3">
        <h3 className="text-xs font-medium text-muted-foreground mb-2 uppercase tracking-wider">
          Backlinks
        </h3>
        {isLoading ? (
          <p className="text-xs text-muted-foreground">Loading...</p>
        ) : backlinks && backlinks.length > 0 ? (
          <ul className="space-y-1">
            {backlinks.map((bl, i) => (
              <li key={i}>
                <button
                  type="button"
                  onClick={() => openFile(bl.source_path)}
                  className="text-sm text-primary hover:underline truncate block w-full text-left"
                >
                  {bl.source_path.replace(/\.md$/, '')}
                </button>
              </li>
            ))}
          </ul>
        ) : (
          <p className="text-xs text-muted-foreground">No backlinks</p>
        )}
      </div>
    </div>
  )
}
