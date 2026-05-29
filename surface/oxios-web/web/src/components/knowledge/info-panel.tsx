import { PanelRightClose } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { useKnowledgeBacklinks, useKnowledgeFileHistory, useKnowledgeFileRestore } from '@/hooks/use-knowledge'
import { cn } from '@/lib/utils'
import { useKnowledgeStore } from '@/stores/knowledge'
import { Copilot } from './copilot'
import { LinkGraph } from './link-graph'

type Tab = 'backlinks' | 'copilot' | 'graph' | 'history'

export function InfoPanel() {
  const { t } = useTranslation()
  const { currentFilePath, toggleInfoPanel, openFile } = useKnowledgeStore()
  const { data: backlinks, isLoading } = useKnowledgeBacklinks(currentFilePath)
  const [tab, setTab] = useState<Tab>('backlinks')

  return (
    <div className="w-80 border-l bg-sidebar text-sidebar-foreground flex flex-col shrink-0 max-md:fixed max-md:inset-0 max-md:z-50 max-md:w-full max-md:border-l-0">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-2.5 border-b">
        <span className="text-sm font-medium">Info</span>
        <Button variant="ghost" size="icon" className="h-7 w-7" onClick={toggleInfoPanel}>
          <PanelRightClose className="h-4 w-4" />
        </Button>
      </div>

      {/* Tabs */}
      <div className="flex border-b">
        {(['backlinks', 'copilot', 'graph', 'history'] as Tab[]).map((t) => (
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
              {t('knowledge.backlinks')}
            </h3>
            {isLoading ? (
              <p className="text-xs text-muted-foreground">{t('knowledge.loading')}</p>
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
              <p className="text-xs text-muted-foreground">{t('knowledge.noBacklinks')}</p>
            )}
          </div>
        )}
        {tab === 'copilot' && <Copilot />}
        {tab === 'graph' && (
          <div className="p-3">
            <LinkGraph />
          </div>
        )}
        {tab === 'history' && (
          <div className="p-3">
            <h3 className="text-xs font-medium text-muted-foreground mb-2 uppercase tracking-wider">
              {t('knowledge.versionHistory', 'Version History')}
            </h3>
            <FileHistoryPanel />
          </div>
        )}
      </div>
    </div>
  )
}

/** File history sub-component */
function FileHistoryPanel() {
  const { currentFilePath } = useKnowledgeStore()
  const { data, isLoading } = useKnowledgeFileHistory(currentFilePath)
  const restore = useKnowledgeFileRestore()
  const { t } = useTranslation()

  if (!currentFilePath) {
    return <p className="text-xs text-muted-foreground">{t('knowledge.noFileOpen', 'No file open')}</p>
  }

  if (isLoading) {
    return <p className="text-xs text-muted-foreground">{t('knowledge.loading', 'Loading...')}</p>
  }

  if (!data || data.history.length === 0) {
    return (
      <p className="text-xs text-muted-foreground">
        {t('knowledge.noHistory', 'No version history yet')}
      </p>
    )
  }

  return (
    <ul className="space-y-2">
      {data.history.map((entry) => (
        <li key={entry.short_hash} className="group">
          <div className="flex items-center justify-between gap-2">
            <div className="min-w-0 flex-1">
              <p className="text-xs text-muted-foreground truncate">
                {entry.short_hash} · {formatTimestamp(entry.timestamp)}
              </p>
              <p className="text-sm truncate">{entry.message}</p>
            </div>
            <Button
              variant="ghost"
              size="sm"
              className="h-6 px-2 text-xs opacity-0 group-hover:opacity-100 transition-opacity shrink-0"
              onClick={() =>
                restore.mutate({ path: currentFilePath, hash: entry.hash })
              }
              disabled={restore.isPending}
              title={t('knowledge.restoreVersion', 'Restore this version')}
            >
              {t('knowledge.restore', 'Restore')}
            </Button>
          </div>
        </li>
      ))}
    </ul>
  )
}

/** Format a git timestamp to a human-readable relative time */
function formatTimestamp(ts: string): string {
  try {
    const date = new Date(ts)
    const now = new Date()
    const diffMs = now.getTime() - date.getTime()
    const diffMin = Math.floor(diffMs / 60000)
    if (diffMin < 1) return 'just now'
    if (diffMin < 60) return `${diffMin}m ago`
    const diffHr = Math.floor(diffMin / 60)
    if (diffHr < 24) return `${diffHr}h ago`
    const diffDay = Math.floor(diffHr / 24)
    if (diffDay < 30) return `${diffDay}d ago`
    return date.toLocaleDateString()
  } catch {
    return ts
  }
}
