import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { Check, PackagePlus, Power, Search, Store, Trash2, X, Zap } from 'lucide-react'
import { useCallback, useDeferredValue, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { EmptyState } from '@/components/shared/empty-state'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { RefreshButton } from '@/components/shared/refresh-button'
import { SkillDetail } from '@/components/skills/skill-detail'
import { MarketplaceDetail } from '@/components/skills/marketplace-detail'
import { UpdateBadge, useSkillUpdates } from '@/components/skills/update-badge'
import { useToast } from '@/components/ui/sonner'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { api } from '@/lib/api-client'
import { cn } from '@/lib/utils'
import type { ClawHubSearchResult, Skill, SkillFormat, SkillStatus, SkillsShSkill } from '@/types'

export const Route = createFileRoute('/skills')({
  component: SkillsPage,
  validateSearch: (search: Record<string, unknown>) => ({
    tab: (search.tab as string) || undefined,
  }),
})

type Tab = 'installed' | 'marketplace'
type MarketplaceSource = 'clawhub' | 'skills-sh'

const STATUS_DISPLAY: Record<SkillStatus, { emoji: string; label: string; variant: 'success' | 'warning' | 'destructive' }> = {
  ready: { emoji: '🟢', label: 'ready', variant: 'success' },
  needs_setup: { emoji: '🟡', label: 'needs-setup', variant: 'warning' },
  disabled: { emoji: '🔴', label: 'disabled', variant: 'destructive' },
}

const SOURCE_VARIANT: Record<string, 'outline' | 'secondary' | 'default'> = {
  managed: 'outline', bundled: 'secondary', workspace: 'default',
}

const FORMAT_META: Record<SkillFormat, { label: string; variant: 'default' | 'secondary' | 'outline'; description: string }> = {
  oxios: { label: 'Oxios', variant: 'default', description: 'Oxios native skill' },
  openclaw: { label: 'OpenClaw', variant: 'secondary', description: 'ClawHub marketplace skill' },
  claude_code: { label: 'Claude', variant: 'outline', description: 'Claude Code skill — core instructions compatible, some features may not apply' },
  agent_skills: { label: 'Standard', variant: 'outline', description: 'Agent Skills standard (agentskills.io)' },
}

function FormatBadge({ format }: { format: SkillFormat }) {
  const m = FORMAT_META[format]
  return <Badge variant={m.variant} className="text-xs" title={m.description}>{m.label}</Badge>
}

// ─── Page ─────────────────────────────────────────────────────

function SkillsPage() {
  const { t } = useTranslation()
  const search = Route.useSearch()
  const [tab, setTab] = useState<Tab>(search.tab === 'marketplace' ? 'marketplace' : 'installed')
  const [mktSource, setMktSource] = useState<MarketplaceSource>('clawhub')
  const [filter, setFilter] = useState<'all' | SkillStatus>('all')
  const [searchQuery, setSearchQuery] = useState('')
  const [mktQuery, setMktQuery] = useState('')
  const deferredQuery = useDeferredValue(mktQuery)

  // Selected skill for detail panel
  const [selectedSkill, setSelectedSkill] = useState<Skill | null>(null)
  const [selectedMktSlug, setSelectedMktSlug] = useState<string | null>(null)
  const [selectedSkillsShId, setSelectedSkillsShId] = useState<string | null>(null)

  const { data: skills, isLoading: sl, isError: se, refetch: sr, isFetching: sf } = useQuery({
    queryKey: ['skills'],
    queryFn: async () => { const r = await api.get<{ skills: Skill[] }>('/api/skills'); return r.skills ?? [] },
    refetchInterval: 30000,
  })
  const { data: mktResults, isLoading: ml, isError: me, refetch: mr } = useQuery({
    queryKey: ['marketplace', 'search', deferredQuery],
    queryFn: async () => { const r = await api.get<ClawHubSearchResult[]>('/api/marketplace/search', { q: deferredQuery }); return r ?? [] },
    enabled: tab === 'marketplace' && mktSource === 'clawhub' && deferredQuery.trim().length > 0,
    refetchOnWindowFocus: false,
  })
  // Skills.sh search
  const { data: skillsShResults, isLoading: ssl, isError: sse, refetch: ssr } = useQuery({
    queryKey: ['skills-sh', 'search', deferredQuery],
    queryFn: async () => { const r = await api.get<{ data: SkillsShSkill[] }>('/api/marketplace/skills-sh/search', { q: deferredQuery }); return r?.data ?? [] },
    enabled: tab === 'marketplace' && mktSource === 'skills-sh' && deferredQuery.trim().length > 0,
    refetchOnWindowFocus: false,
  })
  // Skills.sh trending list (loaded when tab is open and source is skills-sh)
  const { data: skillsShTrending } = useQuery({
    queryKey: ['skills-sh', 'trending'],
    queryFn: async () => { const r = await api.get<{ data: SkillsShSkill[] }>('/api/marketplace/skills-sh/list', { view: 'trending', per_page: 20 }); return r?.data ?? [] },
    enabled: tab === 'marketplace' && mktSource === 'skills-sh',
    refetchOnWindowFocus: false,
    staleTime: 60_000,
  })

  // Updates check
  const { data: updates } = useSkillUpdates()
  const updateCount = updates?.length ?? 0

  const counts = useMemo(() => {
    const l = skills ?? []
    return { all: l.length, ready: l.filter(s => s.status === 'ready').length, needs_setup: l.filter(s => s.status === 'needs_setup').length, disabled: l.filter(s => s.status === 'disabled').length }
  }, [skills])

  const filtered = useMemo(() => {
    let l = skills ?? []
    if (filter !== 'all') l = l.filter(s => s.status === filter)
    if (searchQuery.trim()) { const q = searchQuery.toLowerCase(); l = l.filter(s => s.name.toLowerCase().includes(q) || s.description.toLowerCase().includes(q)) }
    return l
  }, [skills, filter, searchQuery])

  // Close detail panel when switching tabs
  const handleTabChange = useCallback((newTab: Tab) => {
    setTab(newTab)
    setSelectedSkill(null)
    setSelectedMktSlug(null)
    setSelectedSkillsShId(null)
  }, [])

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold flex items-center gap-2"><Zap className="h-6 w-6" /> {t('skills.title')}</h1>
          <p className="text-muted-foreground">{t('skills.subtitle')}</p>
        </div>
        <RefreshButton onClick={() => { sr(); if (tab === 'marketplace') mr() }} isFetching={sf} />
      </div>

      {/* Tab switcher */}
      <div className="inline-flex h-9 items-center rounded-lg bg-muted p-1 text-muted-foreground gap-0.5">
        <button onClick={() => handleTabChange('installed')} className={cn('inline-flex items-center justify-center whitespace-nowrap rounded-md px-3 py-1 text-sm font-medium transition-all gap-1.5', tab === 'installed' ? 'bg-background text-foreground shadow' : 'hover:bg-background/50')}>
          <Zap className="h-3.5 w-3.5" /> {t('skills.installed')} <span className="text-xs text-muted-foreground">{counts.all}</span>
        </button>
        <button onClick={() => handleTabChange('marketplace')} className={cn('inline-flex items-center justify-center whitespace-nowrap rounded-md px-3 py-1 text-sm font-medium transition-all gap-1.5', tab === 'marketplace' ? 'bg-background text-foreground shadow' : 'hover:bg-background/50')}>
          <Store className="h-3.5 w-3.5" /> {t('skills.marketplace')}
          <UpdateBadge count={updateCount} />
        </button>
      </div>

      {/* Main content area with optional side panel */}
      <div className={cn('grid gap-6', (selectedSkill || selectedMktSlug || selectedSkillsShId) ? 'grid-cols-1 lg:grid-cols-[1fr_380px]' : 'grid-cols-1')}>
        <div>
          {tab === 'installed' ? (
            <InstalledTab
              filtered={filtered}
              allSkills={skills ?? []}
              counts={counts}
              filter={filter}
              setFilter={setFilter}
              search={searchQuery}
              setSearch={setSearchQuery}
              isLoading={sl}
              isError={se}
              refetch={sr}
              selectedSkill={selectedSkill}
              onSelectSkill={setSelectedSkill}
              updates={updates}
            />
          ) : (
            <MarketplaceTab
              source={mktSource}
              onSourceChange={setMktSource}
              clawhubResults={mktResults}
              skillsShResults={deferredQuery.trim() ? skillsShResults : skillsShTrending}
              query={mktQuery}
              setQuery={setMktQuery}
              deferredQuery={deferredQuery}
              isLoading={mktSource === 'clawhub' ? ml : ssl}
              isError={mktSource === 'clawhub' ? me : sse}
              refetch={() => { mr(); ssr() }}
              selectedClawhubSlug={selectedMktSlug}
              onSelectClawhubSlug={setSelectedMktSlug}
              selectedSkillsShId={selectedSkillsShId}
              onSelectSkillsShId={setSelectedSkillsShId}
            />
          )}
        </div>

        {/* Side panel */}
        {selectedSkill && (
          <div className="border rounded-lg p-4 h-fit sticky top-6">
            <SkillDetail skill={selectedSkill} onClose={() => setSelectedSkill(null)} />
          </div>
        )}
        {selectedMktSlug && (
          <div className="border rounded-lg p-4 h-fit sticky top-6">
            <MarketplaceDetail slug={selectedMktSlug} onClose={() => setSelectedMktSlug(null)} />
          </div>
        )}
        {selectedSkillsShId && (
          <div className="border rounded-lg p-4 h-fit sticky top-6">
            <SkillsShDetail id={selectedSkillsShId} onClose={() => setSelectedSkillsShId(null)} />
          </div>
        )}
      </div>
    </div>
  )
}

// ─── Installed Tab ────────────────────────────────────────────

interface SkillUpdate {
  slug: string
  currentVersion: string
  latestVersion: string
  changelog?: string
}

function InstalledTab({ filtered, allSkills, counts, filter, setFilter, search, setSearch, isLoading, isError, refetch, selectedSkill, onSelectSkill, updates }: {
  filtered: Skill[]; allSkills: Skill[]; counts: Record<string, number>; filter: 'all' | SkillStatus; setFilter: (f: 'all' | SkillStatus) => void; search: string; setSearch: (s: string) => void; isLoading: boolean; isError: boolean; refetch: () => void; selectedSkill: Skill | null; onSelectSkill: (s: Skill | null) => void; updates?: SkillUpdate[]
}) {
  const { t } = useTranslation()
  const qc = useQueryClient()
  const { toast } = useToast()
  const [deleteTarget, setDeleteTarget] = useState<Skill | null>(null)

  const toggleMutation = useMutation({
    mutationFn: ({ name, enable }: { name: string; enable: boolean }) => {
      const endpoint = enable
        ? `/api/skills/${encodeURIComponent(name)}/enable`
        : `/api/skills/${encodeURIComponent(name)}/disable`
      return api.post(endpoint)
    },
    onSuccess: () => {
      toast(t('skills.toggleSuccess'), 'success')
      qc.invalidateQueries({ queryKey: ['skills'] })
    },
    onError: (err: unknown) => {
      toast(err instanceof Error ? err.message : t('common.error'), 'destructive')
    },
  })

  const deleteMutation = useMutation({
    mutationFn: (name: string) => api.delete(`/api/skills/${encodeURIComponent(name)}`),
    onSuccess: () => {
      toast(t('skills.deleteSuccess', { name: deleteTarget?.name }), 'success')
      qc.invalidateQueries({ queryKey: ['skills'] })
      setDeleteTarget(null)
      if (selectedSkill && deleteTarget && selectedSkill.name === deleteTarget.name) {
        onSelectSkill(null)
      }
    },
    onError: (err: unknown) => {
      toast(err instanceof Error ? err.message : t('common.error'), 'destructive')
    },
  })

  if (isLoading) return <LoadingCards count={4} />
  if (isError) return <ErrorState onRetry={() => refetch()} />

  return (<>
    <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
      <div className="inline-flex h-9 items-center rounded-lg bg-muted p-1 text-muted-foreground gap-0.5">
        {([{ key: 'all' as const, labelKey: 'common.all' }, { key: 'ready' as const, labelKey: 'skills.statusReady' }, { key: 'needs_setup' as const, labelKey: 'skills.statusNeedsSetup' }, { key: 'disabled' as const, labelKey: 'common.disabled' }] as const).map(ti => (
          <button key={ti.key} onClick={() => setFilter(ti.key)} className={cn('inline-flex items-center justify-center whitespace-nowrap rounded-md px-3 py-1 text-sm font-medium transition-all', filter === ti.key ? 'bg-background text-foreground shadow' : 'hover:bg-background/50')}>
            {t(ti.labelKey)} <span className="ml-1.5 text-xs text-muted-foreground">{counts[ti.key === 'needs_setup' ? 'needs_setup' : ti.key]}</span>
          </button>
        ))}
      </div>
      <Input placeholder={t('skills.searchInstalled')} value={search} onChange={e => setSearch(e.target.value)} className="max-w-xs" />
    </div>
    {filtered.length === 0 ? (
      <EmptyState icon={<Zap className="h-10 w-10" />} title={allSkills.length === 0 ? t('skills.noSkills') : t('skills.noMatching')} description={allSkills.length === 0 ? t('skills.noSkillsDescription') : t('skills.noMatchingDescription')} />
    ) : (
      <div className="grid gap-4">{filtered.map(s => {
        const hasUpdate = updates?.some(u => u.slug === s.name)
        return (
          <SkillCard
            key={s.name}
            skill={s}
            isSelected={selectedSkill?.name === s.name}
            hasUpdate={hasUpdate}
            onSelect={() => onSelectSkill(selectedSkill?.name === s.name ? null : s)}
            onToggle={() => toggleMutation.mutate({ name: s.name, enable: s.status === 'disabled' })}
            onDelete={() => setDeleteTarget(s)}
            isToggling={toggleMutation.isPending}
          />
        )
      })}</div>
    )}

    {/* Delete confirmation dialog */}
    <Dialog open={!!deleteTarget} onOpenChange={(open) => { if (!open) setDeleteTarget(null) }}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t('skills.deleteConfirm')}</DialogTitle>
          <DialogDescription>
            {t('skills.deleteDescription', { name: deleteTarget?.name ?? '' })}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" size="sm" onClick={() => setDeleteTarget(null)}>
            {t('common.cancel')}
          </Button>
          <Button
            variant="destructive"
            size="sm"
            onClick={() => deleteTarget && deleteMutation.mutate(deleteTarget.name)}
            disabled={deleteMutation.isPending}
          >
            {t('common.delete')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </>)
}

// ─── Marketplace Tab ──────────────────────────────────────────

function MarketplaceTab({ source, onSourceChange, clawhubResults, skillsShResults, query, setQuery, deferredQuery, isLoading, isError, refetch, selectedClawhubSlug, onSelectClawhubSlug, selectedSkillsShId, onSelectSkillsShId }: {
  source: MarketplaceSource
  onSourceChange: (s: MarketplaceSource) => void
  clawhubResults?: ClawHubSearchResult[]
  skillsShResults?: SkillsShSkill[]
  query: string
  setQuery: (s: string) => void
  deferredQuery: string
  isLoading: boolean
  isError: boolean
  refetch: () => void
  selectedClawhubSlug: string | null
  onSelectClawhubSlug: (s: string | null) => void
  selectedSkillsShId: string | null
  onSelectSkillsShId: (s: string | null) => void
}) {
  const { t } = useTranslation()
  const qc = useQueryClient()
  const { toast } = useToast()

  // ClawHub install mutation
  const clawhubMut = useMutation({
    mutationFn: ({ slug, version }: { slug: string; version?: string }) => api.post('/api/marketplace/skills/' + slug + '/install', { version }),
    onSuccess: (_: unknown, v: { slug: string; version?: string }) => { toast(t('skills.installSuccess', { slug: v.slug }), 'success'); qc.invalidateQueries({ queryKey: ['skills'] }) },
    onError: (e: unknown) => { toast(e instanceof Error ? e.message : t('skills.installFailed'), 'destructive') },
  })

  // Skills.sh install mutation
  const skillsShMut = useMutation({
    mutationFn: (id: string) => api.post('/api/marketplace/skills-sh/skill/' + encodeURIComponent(id) + '/install'),
    onSuccess: (_: unknown, id: string) => { toast(t('skills.installSuccess', { slug: id }), 'success'); qc.invalidateQueries({ queryKey: ['skills'] }) },
    onError: (e: unknown) => { toast(e instanceof Error ? e.message : t('skills.installFailed'), 'destructive') },
  })

  const hasQ = deferredQuery.trim().length > 0

  return (<>
    {/* Source toggle */}
    <div className="flex items-center gap-3">
      <div className="relative flex-1">
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground pointer-events-none" />
        <Input placeholder={t('skills.searchMarketplace')} value={query} onChange={e => setQuery(e.target.value)} className="pl-10" autoFocus />
      </div>
      <div className="inline-flex h-9 items-center rounded-lg bg-muted p-1 text-muted-foreground gap-0.5">
        <button onClick={() => onSourceChange('clawhub')} className={cn('inline-flex items-center justify-center whitespace-nowrap rounded-md px-2.5 py-1 text-xs font-medium transition-all gap-1', source === 'clawhub' ? 'bg-background text-foreground shadow' : 'hover:bg-background/50')}>
          ClawHub
        </button>
        <button onClick={() => onSourceChange('skills-sh')} className={cn('inline-flex items-center justify-center whitespace-nowrap rounded-md px-2.5 py-1 text-xs font-medium transition-all gap-1', source === 'skills-sh' ? 'bg-background text-foreground shadow' : 'hover:bg-background/50')}>
          Skills.sh
          <span className="text-[10px] text-muted-foreground">npx</span>
        </button>
      </div>
    </div>

    {/* Results */}
    {source === 'clawhub' ? (
      !hasQ ? (
        <EmptyState icon={<PackagePlus className="h-10 w-10" />} title={t('skills.discover')} description={t('skills.discoverDescription')} />
      ) : isLoading ? <LoadingCards count={4} /> : isError ? <ErrorState onRetry={() => refetch()} /> : clawhubResults?.length === 0 ? (
        <EmptyState icon={<Search className="h-10 w-10" />} title={t('skills.noResults')} description={t('skills.noResultsFor', { query: deferredQuery })} />
      ) : (
        <div className="grid gap-4">{clawhubResults!.map(s => (
          <MarketplaceCard
            key={s.slug}
            skill={s}
            isSelected={selectedClawhubSlug === s.slug}
            isInstalling={clawhubMut.isPending}
            onSelect={() => onSelectClawhubSlug(selectedClawhubSlug === s.slug ? null : s.slug)}
            onInstall={(sl, v) => clawhubMut.mutate({ slug: sl, version: v })}
          />
        ))}</div>
      )
    ) : (
      // Skills.sh
      !hasQ && !skillsShResults?.length ? (
        <EmptyState icon={<PackagePlus className="h-10 w-10" />} title={t('skills.discover')} description={t('skills.discoverDescription')} />
      ) : isLoading ? <LoadingCards count={4} /> : isError ? <ErrorState onRetry={() => refetch()} /> : skillsShResults?.length === 0 ? (
        <EmptyState icon={<Search className="h-10 w-10" />} title={t('skills.noResults')} description={t('skills.noResultsFor', { query: deferredQuery })} />
      ) : (
        <div className="grid gap-4">{skillsShResults!.map(s => (
          <SkillsShCard
            key={s.id}
            skill={s}
            isSelected={selectedSkillsShId === s.id}
            isInstalling={skillsShMut.isPending}
            onSelect={() => onSelectSkillsShId(selectedSkillsShId === s.id ? null : s.id)}
            onInstall={(id) => skillsShMut.mutate(id)}
          />
        ))}</div>
      )
    )}
  </>)
}

// ─── Skill Card (enhanced) ────────────────────────────────────

function SkillCard({ skill, isSelected, hasUpdate, onSelect, onToggle, onDelete, isToggling }: {
  skill: Skill; isSelected: boolean; hasUpdate?: boolean; onSelect: () => void; onToggle: () => void; onDelete: () => void; isToggling: boolean
}) {
  const { t } = useTranslation()
  const sd = STATUS_DISPLAY[skill.status]
  const isClaude = skill.format === 'claude_code'
  const isDisabled = skill.status === 'disabled'
  const hasMissing = skill.missing.bins.length > 0 || skill.missing.anyBins.length > 0 || skill.missing.env.length > 0 || skill.missing.config.length > 0

  return (
    <Card className={cn('transition-shadow hover:shadow-md cursor-pointer', isSelected && 'ring-2 ring-primary')}>
      <CardContent className="p-5 space-y-3">
        <div className="flex items-start justify-between gap-3">
          <div className="flex items-start gap-2 min-w-0" onClick={onSelect}>
            <span className="text-lg leading-none mt-0.5 shrink-0">{skill.emoji || '⚡'}</span>
            <div className="min-w-0">
              <h3 className="font-semibold text-base leading-tight">{skill.name}</h3>
              {skill.description && <p className="text-sm text-muted-foreground mt-0.5 line-clamp-2">{skill.description}</p>}
            </div>
          </div>
          <div className="flex items-center gap-2 shrink-0">
            {hasUpdate && (
              <Badge variant="outline" className="text-xs gap-1 border-amber-500/50 text-amber-600 dark:text-amber-400">
                {t('skills.updateAvailable')}
              </Badge>
            )}
            <FormatBadge format={skill.format} />
            {skill.always && <Badge variant="outline" className="text-xs">{t('skills.always')}</Badge>}
            <Badge variant={sd.variant} className="text-xs gap-1"><span>{sd.emoji}</span> {sd.label}</Badge>
          </div>
        </div>
        <div className="flex items-center gap-2 text-xs text-muted-foreground">
          {skill.version && <span className="font-mono">v{skill.version}</span>}
          <Badge variant={SOURCE_VARIANT[skill.source] ?? 'outline'} className="text-xs">{skill.source}</Badge>
          {skill.author && <span>{t('skills.by')} {skill.author}</span>}
        </div>
        {isClaude && (
          <div className="rounded-md bg-blue-500/10 border border-blue-500/20 px-3 py-2">
            <p className="text-xs text-blue-700 dark:text-blue-400">{t('skills.claudeCompatible')}</p>
          </div>
        )}
        {(skill.requirements.bins.length > 0 || skill.requirements.anyBins.length > 0 || skill.requirements.env.length > 0 || skill.requirements.config.length > 0) && (
          <div className="space-y-1.5">
            <p className="text-xs font-medium text-muted-foreground uppercase tracking-wider">{t('skills.requires')}</p>
            <div className="space-y-1 pl-2">
              {skill.requirements.bins.length > 0 && <ReqRow labelKey="skills.bins" items={skill.requirements.bins} missing={skill.missing.bins} />}
              {skill.requirements.anyBins.length > 0 && <ReqRow labelKey="skills.anyBins" items={skill.requirements.anyBins} missing={skill.missing.anyBins} />}
              {skill.requirements.env.length > 0 && <ReqRow labelKey="skills.env" items={skill.requirements.env} missing={skill.missing.env} />}
              {skill.requirements.config.length > 0 && <ReqRow labelKey="skills.config" items={skill.requirements.config} missing={skill.missing.config} />}
            </div>
          </div>
        )}
        {skill.install.length > 0 && (
          <div className="space-y-1.5">
            <p className="text-xs font-medium text-muted-foreground uppercase tracking-wider">{t('skills.install')}</p>
            <div className="pl-2 space-y-1">{skill.install.map((sp, i) => (
              <div key={`${sp.kind}-${i}`} className="flex items-center gap-2 text-sm text-muted-foreground">
                <span className="text-xs font-mono bg-muted px-1.5 py-0.5 rounded">{sp.kind}</span>
                <span>{sp.label ?? sp.bins.join(', ')}</span>
              </div>
            ))}</div>
          </div>
        )}
        {hasMissing && skill.status === 'needs_setup' && (
          <div className="rounded-md bg-amber-500/10 border border-amber-500/20 px-3 py-2">
            <p className="text-xs text-amber-700 dark:text-amber-400">{t('skills.missingWarning', { missing: [...skill.missing.bins.map(b => `bin:${b}`), ...skill.missing.env.map(e => `env:${e}`), ...skill.missing.config.map(c => `config:${c}`), ...skill.missing.anyBins.map(b => `any_bin:${b}`)].join(', ') })}</p>
          </div>
        )}
        {/* Inline actions */}
        <div className="flex items-center gap-2 pt-2 border-t" onClick={(e) => e.stopPropagation()}>
          <Button
            variant={isDisabled ? 'default' : 'outline'}
            size="sm"
            onClick={onToggle}
            disabled={isToggling}
            className="gap-1.5"
          >
            <Power className="h-3.5 w-3.5" />
            {isDisabled ? t('skills.enable') : t('skills.disable')}
          </Button>
          <Button
            variant="ghost"
            size="sm"
            onClick={onDelete}
            className="gap-1.5 text-destructive hover:text-destructive"
          >
            <Trash2 className="h-3.5 w-3.5" />
            {t('skills.delete')}
          </Button>
        </div>
      </CardContent>
    </Card>
  )
}

// ─── Marketplace Card (enhanced) ──────────────────────────────

function MarketplaceCard({ skill, isSelected, isInstalling, onSelect, onInstall }: { skill: ClawHubSearchResult; isSelected: boolean; isInstalling: boolean; onSelect: () => void; onInstall: (s: string, v?: string) => void }) {
  const { t } = useTranslation()
  const v = skill.version; const dn = skill.displayName || skill.slug
  const rt = useMemo(() => { if (!skill.updatedAt) return null; const d = Math.floor((Date.now() - skill.updatedAt) / 86_400_000); if (d === 0) return 'today'; if (d === 1) return '1d ago'; if (d < 30) return `${d}d ago`; const w = Math.floor(d / 7); if (w < 4) return `${w}w ago`; return `${Math.floor(d / 30)}mo ago` }, [skill.updatedAt])
  return (
    <Card className={cn('transition-shadow hover:shadow-md cursor-pointer', isSelected && 'ring-2 ring-primary')}>
      <CardContent className="p-5 space-y-3">
        <div className="flex items-start justify-between gap-3">
          <div className="flex items-start gap-2 min-w-0" onClick={onSelect}>
            <span className="text-lg leading-none mt-0.5 shrink-0">🔍</span>
            <div className="min-w-0">
              <h3 className="font-semibold text-base leading-tight">{dn}</h3>
              {skill.summary && <p className="text-sm text-muted-foreground mt-0.5 line-clamp-2">{skill.summary}</p>}
            </div>
          </div>
          <div className="flex items-center gap-2 shrink-0">
            <FormatBadge format="openclaw" />
            {v && <Badge variant="outline" className="text-xs font-mono">v{v}</Badge>}
            <Button size="sm" onClick={(e) => { e.stopPropagation(); onInstall(skill.slug, v) }} disabled={isInstalling} className="gap-1.5">{t('skills.install')}</Button>
          </div>
        </div>
        <div className="flex items-center gap-2 text-xs text-muted-foreground">
          <span className="font-mono text-muted-foreground/80">{skill.slug}</span>
          {rt && <><span>·</span><span>{rt}</span></>}
        </div>
      </CardContent>
    </Card>
  )
}

// ─── Skills.sh Card ────────────────────────────────────────────

function SkillsShCard({ skill, isSelected, isInstalling, onSelect, onInstall }: { skill: SkillsShSkill; isSelected: boolean; isInstalling: boolean; onSelect: () => void; onInstall: (id: string) => void }) {
  const { t } = useTranslation()
  return (
    <Card className={cn('transition-shadow hover:shadow-md cursor-pointer', isSelected && 'ring-2 ring-primary')}>
      <CardContent className="p-5 space-y-3">
        <div className="flex items-start justify-between gap-3">
          <div className="flex items-start gap-2 min-w-0" onClick={onSelect}>
            <span className="text-lg leading-none mt-0.5 shrink-0">🌐</span>
            <div className="min-w-0">
              <h3 className="font-semibold text-base leading-tight">{skill.name}</h3>
              <p className="text-sm text-muted-foreground mt-0.5 line-clamp-2">{skill.source}</p>
            </div>
          </div>
          <div className="flex items-center gap-2 shrink-0">
            <Badge variant="secondary" className="text-xs">Skills.sh</Badge>
            <Badge variant="outline" className="text-xs font-mono">{skill.installs.toLocaleString()}</Badge>
            <Button size="sm" onClick={(e) => { e.stopPropagation(); onInstall(skill.id) }} disabled={isInstalling} className="gap-1.5">{t('skills.install')}</Button>
          </div>
        </div>
        <div className="flex items-center gap-2 text-xs text-muted-foreground">
          <span className="font-mono text-muted-foreground/80">{skill.slug}</span>
          <span>·</span>
          <span>{skill.source}</span>
          {skill.sourceType === 'github' && <Badge variant="outline" className="text-[10px] px-1 py-0">GitHub</Badge>}
          {skill.isDuplicate && <Badge variant="outline" className="text-[10px] px-1 py-0 border-amber-500/50 text-amber-600">fork</Badge>}
        </div>
      </CardContent>
    </Card>
  )
}

// ─── Skills.sh Detail Panel ────────────────────────────────────────

function SkillsShDetail({ id, onClose }: { id: string; onClose: () => void }) {
  const { t } = useTranslation()
  const { toast } = useToast()
  const qc = useQueryClient()

  const { data, isLoading, isError } = useQuery({
    queryKey: ['skills-sh', 'detail', id],
    queryFn: async () => {
      const r = await api.get<{ id: string; source: string; slug: string; installs: number; hash?: string; files?: Array<{ path: string; contents: string }> }>(
        '/api/marketplace/skills-sh/skill/' + encodeURIComponent(id)
      )
      return r
    },
    refetchOnWindowFocus: false,
  })

  const installMut = useMutation({
    mutationFn: () => api.post('/api/marketplace/skills-sh/skill/' + encodeURIComponent(id) + '/install'),
    onSuccess: () => { toast(t('skills.installSuccess', { slug: id }), 'success'); qc.invalidateQueries({ queryKey: ['skills'] }) },
    onError: (e: unknown) => { toast(e instanceof Error ? e.message : t('skills.installFailed'), 'destructive') },
  })

  if (isLoading) return <div className="text-sm text-muted-foreground">Loading...</div>
  if (isError || !data) return <div className="text-sm text-destructive">Failed to load skill detail</div>

  const skillMd = data.files?.find(f => f.path === 'SKILL.md' || f.path.toLowerCase() === 'skill.md')

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h3 className="font-semibold text-lg">{data.slug}</h3>
        <Button variant="ghost" size="sm" onClick={onClose}>✕</Button>
      </div>

      <div className="space-y-2">
        <div className="flex items-center gap-2 text-sm">
          <Badge variant="secondary">Skills.sh</Badge>
          <span className="text-muted-foreground">{data.source}</span>
        </div>
        <div className="text-sm text-muted-foreground">
          {data.installs.toLocaleString()} installs
          {data.hash && <span className="ml-2 font-mono text-xs">({data.hash.slice(0, 8)}...)</span>}
        </div>
      </div>

      {data.files && data.files.length > 0 && (
        <div className="space-y-1">
          <p className="text-xs font-medium text-muted-foreground uppercase tracking-wider">Files</p>
          <div className="space-y-1">
            {data.files.map(f => (
              <div key={f.path} className="text-xs font-mono bg-muted px-2 py-1 rounded">
                {f.path} <span className="text-muted-foreground">({f.contents.length} chars)</span>
              </div>
            ))}
          </div>
        </div>
      )}

      {skillMd && (
        <div className="space-y-1">
          <p className="text-xs font-medium text-muted-foreground uppercase tracking-wider">SKILL.md Preview</p>
          <pre className="text-xs bg-muted p-3 rounded-md overflow-auto max-h-64 whitespace-pre-wrap">{skillMd.contents.slice(0, 2000)}{skillMd.contents.length > 2000 ? '\n...' : ''}</pre>
        </div>
      )}

      <Button className="w-full" onClick={() => installMut.mutate()} disabled={installMut.isPending}>
        {t('skills.install')}
      </Button>
    </div>
  )
}

// ─── Helpers ─────────────────────────────────────────────────

function ReqRow({ labelKey, items, missing }: { labelKey: string; items: string[]; missing: string[] }) {
  const { t } = useTranslation()
  return (
    <div className="flex items-start gap-2 text-xs">
      <span className="text-muted-foreground w-16 shrink-0 pt-px">{t(labelKey)}</span>
      <div className="flex flex-wrap gap-x-3 gap-y-0.5">
        {items.map(item => { const m = missing.includes(item); return (
          <span key={item} className={cn('flex items-center gap-1', m ? 'text-red-600 dark:text-red-400' : 'text-emerald-600 dark:text-emerald-400')}>
            {m ? <X className="h-3 w-3" /> : <Check className="h-3 w-3" />}
            {item}{m && <span className="text-red-400 dark:text-red-500">{t('skills.missing')}</span>}
          </span>
        )})}
      </div>
    </div>
  )
}
