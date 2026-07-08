import { PanelRightClose, Plus, Unlink } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { EventEditor } from '@/components/calendar/event-editor'
import { Button } from '@/components/ui/button'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { useCalendarByNote, useCalendarCreate, useCalendarUpdate } from '@/hooks/use-calendar'
import {
  useKnowledgeBacklinks,
  useKnowledgeFileHistory,
  useKnowledgeFileRestore,
} from '@/hooks/use-knowledge'
import { isSubsystemUnavailable } from '@/lib/api-client'
import { useKnowledgeStore } from '@/stores/knowledge'
import { useNotificationCenter } from '@/stores/notification-center'
import { useHour12 } from '@/stores/ui-prefs'
import type { CreateEventRequest } from '@/types/calendar'
import { Copilot } from './copilot'
import { LinkGraph } from './link-graph'

type Tab = 'backlinks' | 'copilot' | 'graph' | 'history' | 'calendar'

export function InfoPanel() {
  const { t } = useTranslation()
  const { currentFilePath, toggleInfoPanel, openFile } = useKnowledgeStore()
  const { data: backlinks, isLoading } = useKnowledgeBacklinks(currentFilePath)
  const [tab, setTab] = useState<Tab>('backlinks')

  return (
    <div className="w-80 border-l bg-sidebar text-sidebar-foreground flex flex-col shrink-0 max-md:fixed max-md:inset-0 max-md:z-50 max-md:w-full max-md:border-l-0">
      {/* Mobile backdrop */}
      <div className="md:hidden fixed inset-0 bg-black/50 -z-10" onClick={toggleInfoPanel} />

      {/* Header */}
      <div className="flex items-center justify-between px-4 py-2.5 border-b">
        <span className="text-sm font-medium">{t('knowledge.info')}</span>
        <Button
          variant="ghost"
          size="icon"
          className="h-8 w-8"
          onClick={toggleInfoPanel}
          aria-label={t('common.close')}
        >
          <PanelRightClose className="h-4 w-4" />
        </Button>
      </div>

      {/* Tabs */}
      <Tabs
        value={tab}
        onValueChange={(v) => setTab(v as Tab)}
        className="shrink-0 px-2 pt-2 border-b"
      >
        <TabsList variant="line" className="w-full overflow-x-auto">
          {(['backlinks', 'copilot', 'graph', 'history', 'calendar'] as Tab[]).map((tabValue) => (
            <TabsTrigger key={tabValue} value={tabValue} className="capitalize flex-none text-xs px-2.5">
              {tabValue}
            </TabsTrigger>
          ))}
        </TabsList>
      </Tabs>

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
        {tab === 'calendar' && (
          <div className="p-3">
            <CalendarPanel />
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
    return (
      <p className="text-xs text-muted-foreground">{t('knowledge.noFileOpen', 'No file open')}</p>
    )
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
              onClick={() => restore.mutate({ path: currentFilePath, hash: entry.hash })}
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

/** Calendar panel — events linked to the current note (note ↔ calendar bridge). */
function CalendarPanel() {
  const { t, i18n } = useTranslation()
  const { currentFilePath } = useKnowledgeStore()
  const hour12 = useHour12()
  const openCenter = useNotificationCenter((s) => s.openCenter)
  const [editorOpen, setEditorOpen] = useState(false)

  const { data, isLoading, error } = useCalendarByNote(currentFilePath)
  const createMutation = useCalendarCreate()
  const updateMutation = useCalendarUpdate()

  const events = data?.events ?? []

  if (!currentFilePath) {
    return <p className="text-xs text-muted-foreground">{t('knowledge.noFileSelectedHint')}</p>
  }

  if (isSubsystemUnavailable(error)) {
    return (
      <p className="text-xs text-muted-foreground">{t('notificationCenter.calendarDisabled')}</p>
    )
  }

  function formatEventTime(iso: string): string {
    return new Date(iso).toLocaleString(i18n.language, {
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
      hour12,
    })
  }

  return (
    <>
      <h3 className="text-xs font-medium text-muted-foreground mb-2 uppercase tracking-wider">
        {t('calendar.linkedEvents')}
      </h3>

      {isLoading ? (
        <p className="text-xs text-muted-foreground">{t('knowledge.loading')}</p>
      ) : events.length > 0 ? (
        <ul className="space-y-1 mb-3">
          {events.map((ev) => (
            <li key={ev.uid} className="group flex items-center gap-2">
              <button
                type="button"
                onClick={openCenter}
                className="flex-1 min-w-0 text-left"
                title={t('calendar.openInCalendar')}
              >
                <span className="block truncate text-sm text-primary">{ev.title}</span>
                <span className="text-xs text-muted-foreground">
                  {ev.all_day ? t('calendar.allDay') : formatEventTime(ev.start)}
                </span>
              </button>
              <button
                type="button"
                onClick={() => updateMutation.mutate({ uid: ev.uid, note_path: null })}
                className="shrink-0 text-muted-foreground opacity-0 transition-opacity hover:text-destructive group-hover:opacity-100"
                title={t('calendar.unlink')}
              >
                <Unlink className="h-3.5 w-3.5" />
              </button>
            </li>
          ))}
        </ul>
      ) : (
        <p className="mb-3 text-xs text-muted-foreground">{t('calendar.noLinkedEvents')}</p>
      )}

      <Button
        variant="outline"
        size="sm"
        className="w-full"
        onClick={() => setEditorOpen(true)}
        disabled={createMutation.isPending}
      >
        <Plus className="mr-1.5 h-3.5 w-3.5" />
        {t('calendar.scheduleNote')}
      </Button>

      <EventEditor
        open={editorOpen}
        onClose={() => setEditorOpen(false)}
        defaultStart={new Date()}
        isLoading={createMutation.isPending}
        onSubmit={(formData) => {
          createMutation.mutate(
            { ...(formData as CreateEventRequest), note_path: currentFilePath },
            { onSuccess: () => setEditorOpen(false) },
          )
        }}
      />
    </>
  )
}
