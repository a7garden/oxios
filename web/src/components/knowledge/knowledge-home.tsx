import { useNavigate } from '@tanstack/react-router'
import { BookOpen, CheckCircle2, FilePlus, Inbox, Lightbulb, Network, Zap } from 'lucide-react'
import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import {
  useChatMessages,
  useJournalToday,
  useKnowledgeDoneToday,
  useKnowledgeGraph,
  useKnowledgeTree,
  useWriteFile,
} from '@/hooks/use-knowledge'
import { cn } from '@/lib/utils'
import { useCommandPaletteStore } from '@/stores/command-palette'
import { useKnowledgeStore } from '@/stores/knowledge'

/** Count actual checklist items in the raw inbox stream (skip date headers). */
function useInboxCount(): number {
  const { data } = useChatMessages()
  return useMemo(() => {
    if (!data) return 0
    return data.filter((line) => /^- \[[xX ]\]/.test(line)).length
  }, [data])
}

/** Flatten the tree to count note files (non-dir). */
function useNoteCount(): number {
  const { data } = useKnowledgeTree()
  return useMemo(() => {
    if (!data) return 0
    return data.filter((e) => !e.is_dir).length
  }, [data])
}

// ── Cards ─────────────────────────────────────────────────────

function StatTile({
  icon,
  iconClassName,
  label,
  value,
  sublabel,
  onClick,
  actionLabel,
}: {
  icon: React.ReactNode
  iconClassName?: string
  label: string
  value: React.ReactNode
  sublabel?: string
  onClick?: () => void
  actionLabel?: string
}) {
  return (
    <Card
      className={cn('flex flex-col transition-shadow', onClick && 'cursor-pointer hover:shadow-md')}
      onClick={onClick}
    >
      <CardContent className="flex flex-1 flex-col gap-3 p-4">
        <div className="flex items-center justify-between">
          <span className="text-xs font-medium text-muted-foreground">{label}</span>
          <span
            className={cn(
              'flex h-7 w-7 items-center justify-center rounded-lg bg-muted',
              iconClassName,
            )}
          >
            {icon}
          </span>
        </div>
        <div className="text-2xl font-bold tabular-nums leading-none">{value}</div>
        {sublabel && <p className="text-xs text-muted-foreground">{sublabel}</p>}
        {actionLabel && (
          <div className="mt-auto pt-1">
            <span className="text-xs font-medium text-primary">{actionLabel} →</span>
          </div>
        )}
      </CardContent>
    </Card>
  )
}

// ── Component ─────────────────────────────────────────────────

export function KnowledgeHome() {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const { openChat, openFile } = useKnowledgeStore()
  const openPalette = useCommandPaletteStore((s) => s.openPalette)
  const inboxCount = useInboxCount()
  const noteCount = useNoteCount()

  const { data: doneData } = useKnowledgeDoneToday()
  const { data: journalToday } = useJournalToday()
  const { data: graph } = useKnowledgeGraph()
  const writeFile = useWriteFile()

  const doneCount = doneData?.count ?? 0
  const doneItems = Array.isArray(doneData?.items) ? doneData.items.slice(0, 4) : []
  const graphNodes = graph?.nodes.length ?? 0
  const graphEdges = graph?.edges.length ?? 0
  const hasJournal = !!journalToday?.path

  const handleNewFile = async () => {
    const name = 'New file.md'
    await writeFile.mutateAsync({ path: name, content: `# New file\n\n` })
    openFile(name)
  }

  const handleOpenJournal = () => {
    if (journalToday?.path) openFile(journalToday.path)
  }

  return (
    <div className="flex-1 overflow-y-auto">
      <div className="mx-auto max-w-5xl space-y-5 p-6 sm:p-8">
        {/* Header */}
        <div>
          <h1 className="text-2xl font-semibold tracking-tight">{t('knowledge.homeTitle')}</h1>
          <p className="text-muted-foreground">{t('knowledge.homeSubtitle')}</p>
        </div>

        {/* Capture — click to open the palette in capture mode */}
        <button
          type="button"
          onClick={() => openPalette()}
          className="group flex w-full items-center gap-3 rounded-xl border bg-primary/5 px-4 py-3 text-left transition-colors hover:bg-primary/10 hover:border-primary/30"
        >
          <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-primary/10 text-primary transition-transform group-hover:scale-110">
            <Zap className="h-4 w-4" />
          </span>
          <div className="min-w-0 flex-1">
            <p className="text-sm font-medium text-foreground">{t('knowledge.captureHintTitle')}</p>
            <p className="text-xs text-muted-foreground">{t('knowledge.captureHintBody')}</p>
          </div>
          <kbd className="shrink-0 rounded border bg-background px-1.5 py-0.5 text-xs font-mono text-muted-foreground transition-colors group-hover:border-primary/30">
            ⌘K
          </kbd>
        </button>

        {/* KPI grid */}
        <div className="grid gap-3 grid-cols-2 lg:grid-cols-4">
          {/* Inbox — triage entry point */}
          <StatTile
            icon={<Inbox className="h-4 w-4" />}
            iconClassName={inboxCount > 0 ? 'text-primary' : 'text-muted-foreground'}
            label={t('knowledge.homeInbox')}
            value={inboxCount}
            sublabel={
              inboxCount > 0 ? t('knowledge.homeInboxPending') : t('knowledge.homeInboxEmpty')
            }
            onClick={openChat}
            actionLabel={inboxCount > 0 ? t('knowledge.homeProcess') : t('knowledge.homeView')}
          />

          {/* Done today */}
          <StatTile
            icon={<CheckCircle2 className="h-4 w-4" />}
            iconClassName={doneCount > 0 ? 'text-success' : 'text-muted-foreground'}
            label={t('knowledge.homeDoneToday')}
            value={doneCount}
            sublabel={
              doneItems.length > 0
                ? doneItems.map(String).join(' · ')
                : t('knowledge.nothingCompletedToday')
            }
          />

          {/* Knowledge graph */}
          <StatTile
            icon={<Network className="h-4 w-4" />}
            iconClassName="text-info"
            label={t('knowledge.homeGraph')}
            value={graphNodes}
            sublabel={t('knowledge.homeGraphLinks', { count: graphEdges })}
            onClick={() => navigate({ to: '/knowledge/graph' })}
            actionLabel={graphNodes > 0 ? t('knowledge.homeExplore') : undefined}
          />

          {/* Today's journal */}
          <StatTile
            icon={<BookOpen className="h-4 w-4" />}
            iconClassName={hasJournal ? 'text-success' : 'text-muted-foreground'}
            label={t('knowledge.homeJournal')}
            value={hasJournal ? t('knowledge.homeReady') : '—'}
            sublabel={t('knowledge.homeJournalHint')}
            onClick={hasJournal ? handleOpenJournal : undefined}
            actionLabel={hasJournal ? t('knowledge.homeOpen') : undefined}
          />
        </div>

        {/* Quick actions */}
        <div className="flex flex-wrap items-center gap-2 pt-1">
          <Button variant="outline" size="sm" onClick={handleNewFile}>
            <FilePlus className="mr-1.5 h-4 w-4" />
            {t('knowledge.newFile')}
          </Button>
          <Button variant="outline" size="sm" onClick={openChat}>
            <Inbox className="mr-1.5 h-4 w-4" />
            {t('knowledge.chatTitle')}
          </Button>
          {hasJournal && (
            <Button variant="outline" size="sm" onClick={handleOpenJournal}>
              <BookOpen className="mr-1.5 h-4 w-4" />
              {t('knowledge.toJournal')}
            </Button>
          )}
          <div className="ml-auto flex items-center gap-1.5 text-xs text-muted-foreground">
            <Lightbulb className="h-3.5 w-3.5" />
            {t('knowledge.homeNoteCount', { count: noteCount })}
          </div>
        </div>
      </div>
    </div>
  )
}
