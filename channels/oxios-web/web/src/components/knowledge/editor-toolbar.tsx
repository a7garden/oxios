import { useRouterState } from '@tanstack/react-router'
import { ArrowLeft, ArrowRight, Columns2, PanelRight, Save, X } from 'lucide-react'
import { useEffect, useRef } from 'react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { Tooltip } from '@/components/ui/tooltip'
import { useKnowledgeStore } from '@/stores/knowledge'

export function EditorToolbar() {
  const { t } = useTranslation()
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
    openSplit,
  } = useKnowledgeStore()

  const canGoBack = historyIndex > 0
  const canGoForward = historyIndex < history.length - 1
  const fileName = currentFilePath?.split('/').pop()?.replace(/\.md$/, '') ?? ''

  const handleSave = () => {
    document.dispatchEvent(new CustomEvent('knowledge:save'))
  }

  // ⌘S keyboard shortcut — only on knowledge routes (B5 fix)
  const router = useRouterState()
  const pathnameRef = useRef(router.location.pathname)
  pathnameRef.current = router.location.pathname

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (!pathnameRef.current.startsWith('/knowledge')) return
      if ((e.metaKey || e.ctrlKey) && e.key === 's') {
        e.preventDefault()
        handleSave()
      }
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [handleSave])

  return (
    <div className="flex items-center gap-1 px-3 py-1.5 border-b bg-muted/30 min-h-[40px]">
      <Button
        variant="ghost"
        size="icon"
        className="h-7 w-7"
        onClick={() => goBack()}
        disabled={!canGoBack}
        title={t('knowledge.goBack')}
      >
        <ArrowLeft className="h-4 w-4" />
      </Button>
      <Button
        variant="ghost"
        size="icon"
        className="h-7 w-7"
        onClick={() => goForward()}
        disabled={!canGoForward}
        title={t('knowledge.goForward')}
      >
        <ArrowRight className="h-4 w-4" />
      </Button>

      <span className="text-sm font-medium truncate mx-3">{fileName || 'Knowledge'}</span>

      <div className="flex-1" />

      {/* Save */}
      {currentFilePath && (
        <Tooltip content={t('knowledge.saveWithShortcut')}>
          <Button variant="ghost" size="icon" className="h-7 w-7" onClick={handleSave} title={t('common.save')}>
            <Save className="h-4 w-4" />
          </Button>
        </Tooltip>
      )}

      {/* Split editor */}
      {!splitEditorOpen && currentFilePath && (
        <Tooltip content={t('knowledge.splitView')}>
          <Button
            variant="ghost"
            size="icon"
            className="h-7 w-7"
            onClick={() => openSplit(currentFilePath)}
            title={t('knowledge.openSplitView')}
          >
            <Columns2 className="h-4 w-4" />
          </Button>
        </Tooltip>
      )}

      {/* Close split */}
      {splitEditorOpen && (
        <Tooltip content={t('knowledge.closeSplitWithShortcut')}>
          <Button
            variant="ghost"
            size="icon"
            className="h-7 w-7"
            onClick={closeSplit}
            title={t('knowledge.closeSplit')}
          >
            <X className="h-4 w-4" />
          </Button>
        </Tooltip>
      )}

      {/* Info panel toggle */}
      <Tooltip content={infoPanelOpen ? t('knowledge.hideInfoPanel') : t('knowledge.showInfoPanel')}>
        <Button
          variant="ghost"
          size="icon"
          className="h-7 w-7"
          onClick={toggleInfoPanel}
          title={t('knowledge.toggleInfoPanel')}
        >
          <PanelRight className="h-4 w-4" />
        </Button>
      </Tooltip>
    </div>
  )
}
