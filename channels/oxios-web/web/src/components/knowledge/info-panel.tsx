import { useState } from 'react'
import { PanelRightClose } from 'lucide-react'
import { useKnowledgeStore } from '@/stores/knowledge'
import { useKnowledgeBacklinks } from '@/hooks/use-knowledge'
import { Copilot } from './copilot'
import { LinkGraph } from './link-graph'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'

type Tab = 'backlinks' | 'copilot' | 'graph'

export function InfoPanel() {
  const { currentFilePath, toggleInfoPanel, openFile } = useKnowledgeStore()
  const { data: backlinks, isLoading } = useKnowledgeBacklinks(currentFilePath)
  const [tab, setTab] = useState<Tab>('backlinks')

  return (
    <div className="w-72 border-l bg-sidebar-background text-sidebar-foreground flex flex-col shrink-0">
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-2 border-b">
        <span className="text-sm font-medium">Info</span>
        <Button variant="ghost" size="icon" className="h-7 w-7" onClick={toggleInfoPanel}>
          <PanelRightClose className="h-4 w-4" />
        </Button>
      </div>

      {/* Tabs */}
      <div className="flex border-b">
        {(['backlinks', 'copilot', 'graph'] as Tab[]).map((t) => (
          <button
            key={t}
            type="button"
            onClick={() => setTab(t)}
            className={cn(
              'flex-1 px-2 py-1.5 text-xs font-medium transition-colors capitalize',
              tab === t
                ? 'text-foreground border-b-2 border-primary'
                : 'text-muted-foreground hover:text-foreground',
            )}
          >
            {t}
          </button>
        ))}
      </div>

      {/* Content */}
      <div className="flex-1 overflow-y-auto">
        {tab === 'backlinks' && (
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
                    <p className="text-xs text-muted-foreground truncate">{bl.link_text}</p>
                  </li>
                ))}
              </ul>
            ) : (
              <p className="text-xs text-muted-foreground">No backlinks</p>
            )}
          </div>
        )}
        {tab === 'copilot' && <Copilot />}
        {tab === 'graph' && (
          <div className="p-3">
            <LinkGraph />
          </div>
        )}
      </div>
    </div>
  )
}
