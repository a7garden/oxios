import { useKnowledgeStore } from '@/stores/knowledge'
import { KnowledgeSidebar } from './knowledge-sidebar'
import { EditorPanel } from './editor-panel'
import { KnowledgeChat } from './knowledge-chat'
import { InfoPanel } from './info-panel'
import { SearchModal } from './search-modal'
import { MoveModal } from './move-modal'
import { useKnowledgeShortcuts } from '@/hooks/use-knowledge-shortcuts'

export function KnowledgeLayout() {
  const { mode, sidebarOpen, infoPanelOpen, toggleSidebar } = useKnowledgeStore()

  // Register global keyboard shortcuts
  useKnowledgeShortcuts()

  return (
    <div className="fixed inset-0 z-30 flex overflow-hidden bg-background">
      {/* Thin strip to re-open sidebar */}
      {!sidebarOpen && (
        <button
          type="button"
          onClick={toggleSidebar}
          className="fixed left-0 top-0 z-40 h-full w-[18px] opacity-0 hover:opacity-100 transition-opacity cursor-pointer bg-border/50"
          aria-label="Open sidebar"
        />
      )}

      {/* Sidebar */}
      {sidebarOpen && <KnowledgeSidebar />}

      {/* Main content */}
      <div className="flex flex-1 min-w-0">
        {mode === 'chat' ? <KnowledgeChat /> : <EditorPanel />}
      </div>

      {/* Info panel (right) */}
      {infoPanelOpen && <InfoPanel />}

      {/* Modals */}
      <SearchModal />
      <MoveModal />
    </div>
  )
}
