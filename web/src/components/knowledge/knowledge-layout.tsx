import { useKnowledgeStore } from '@/stores/knowledge'
import { EditorPanel } from './editor-panel'
import { KnowledgeChat } from './knowledge-chat'
import { KnowledgeHome } from './knowledge-home'

/**
 * KnowledgeLayout renders inline within the AppLayout outlet.
 *
 * The AppLayout handles:
 * - Knowledge sidebar (replaces main sidebar)
 * - Info panel (right side)
 * - Search/Move modals
 * - Keyboard shortcuts
 * - Header with Knowledge breadcrumb
 *
 * This component just renders the content area (editor or chat).
 */
export function KnowledgeLayout() {
  const { mode } = useKnowledgeStore()

  return mode === 'chat' ? <KnowledgeChat /> : mode === 'home' ? <KnowledgeHome /> : <EditorPanel />
}
