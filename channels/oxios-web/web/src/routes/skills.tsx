import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { Check, PackagePlus, Search, Store, X, Zap } from 'lucide-react'
import { useCallback, useDeferredValue, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { EmptyState } from '@/components/shared/empty-state'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { RefreshButton } from '@/components/shared/refresh-button'
import { useToast } from '@/components/ui/sonner'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { api } from '@/lib/api-client'
import { cn } from '@/lib/utils'
import type { ClawHubSearchResult, Skill, SkillFormat, SkillStatus } from '@/types'

export const Route = createFileRoute('/skills')({ component: SkillsPage })

type Tab = 'installed' | 'marketplace'

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
  const [tab, setTab] = useState<Tab>('installed')
  const [filter, setFilter] = useState<'all' | SkillStatus>('all')
  const [search, setSearch] = useState('')
  const [mktQuery, setMktQuery] = useState('')
  const deferredQuery = useDeferredValue(mktQuery)

  const { data: skills, isLoading: sl, isError: se, refetch: sr, isFetching: sf } = useQuery({
    queryKey: ['skills'],
    queryFn: async () => { const r = await api.get<{ skills: Skill[] }>('/api/skills'); return r.skills ?? [] },
    refetchInterval: 30000,
  })
  const { data: mktResults, isLoading: ml, isError: me, refetch: mr } = useQuery({
    queryKey: ['marketplace', 'search', deferredQuery],
    queryFn: async () => { const r = await api.get<ClawHubSearchResult[]>('/api/marketplace/search', { q: deferredQuery }); return r ?? [] },
    enabled: tab === 'marketplace' && deferredQuery.trim().length > 0,
    refetchOnWindowFocus: false,
  })

  const counts = useMemo(() => {
    const l = skills ?? []
    return { all: l.length, ready: l.filter(s => s.status === 'ready').length, needs_setup: l.filter(s => s.status === 'needs_setup').length, disabled: l.filter(s => s.status === 'disabled').length }
  }, [skills])

  const filtered = useMemo(() => {
    let l = skills ?? []
    if (filter !== 'all') l = l.filter(s => s.status === filter)
    if (search.trim()) { const q = search.toLowerCase(); l = l.filter(s => s.name.toLowerCase().includes(q) || s.description.toLowerCase().includes(q)) }
    return l
  }, [skills, filter, search])

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
        <button onClick={() => setTab('installed')} className={cn('inline-flex items-center justify-center whitespace-nowrap rounded-md px-3 py-1 text-sm font-medium transition-all gap-1.5', tab === 'installed' ? 'bg-background text-foreground shadow' : 'hover:bg-background/50')}>
          <Zap className="h-3.5 w-3.5" /> {t('skills.installed')} <span className="text-xs text-muted-foreground">{counts.all}</span>
        </button>
        <button onClick={() => setTab('marketplace')} className={cn('inline-flex items-center justify-center whitespace-nowrap rounded-md px-3 py-1 text-sm font-medium transition-all gap-1.5', tab === 'marketplace' ? 'bg-background text-foreground shadow' : 'hover:bg-background/50')}>
          <Store className="h-3.5 w-3.5" /> {t('skills.marketplace')}
        </button>
      </div>

      {tab === 'installed' ? (
        <InstalledTab filtered={filtered} allSkills={skills ?? []} counts={counts} filter={filter} setFilter={setFilter} search={search} setSearch={setSearch} isLoading={sl} isError={se} refetch={sr} />
      ) : (
        <MarketplaceTab results={mktResults} query={mktQuery} setQuery={setMktQuery} deferredQuery={deferredQuery} isLoading={ml} isError={me} refetch={mr} />
      )}
    </div>
  )
}

// ─── Installed Tab ────────────────────────────────────────────

function InstalledTab({ filtered, allSkills, counts, filter, setFilter, search, setSearch, isLoading, isError, refetch }: {
  filtered: Skill[]; allSkills: Skill[]; counts: Record<string, number>; filter: 'all' | SkillStatus; setFilter: (f: 'all' | SkillStatus) => void; search: string; setSearch: (s: string) => void; isLoading: boolean; isError: boolean; refetch: () => void
}) {
  const { t } = useTranslation()

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
      <div className="grid gap-4">{filtered.map(s => <SkillCard key={s.name} skill={s} />)}</div>
    )}
  </>)
}

// ─── Marketplace Tab ──────────────────────────────────────────

function MarketplaceTab({ results, query, setQuery, deferredQuery, isLoading, isError, refetch }: {
  results?: ClawHubSearchResult[]; query: string; setQuery: (s: string) => void; deferredQuery: string; isLoading: boolean; isError: boolean; refetch: () => void
}) {
  const { t } = useTranslation()
  const qc = useQueryClient()
  const { toast } = useToast()
  const mut = useMutation({
    mutationFn: ({ slug, version }: { slug: string; version?: string }) => api.post('/api/marketplace/skills/' + slug + '/install', { version }),
    onSuccess: (_: unknown, v: { slug: string }) => { toast(t('skills.installedSuccess', { slug: v.slug })), 'success'; qc.invalidateQueries({ queryKey: ['skills'] }) },
    onError: (e: unknown) => { toast(e instanceof Error ? e.message : t('skills.installFailed'), 'destructive') },
  })
  const hasQ = deferredQuery.trim().length > 0

  return (<>
    <div className="relative">
      <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground pointer-events-none" />
      <Input placeholder={t('skills.searchMarketplace')} value={query} onChange={e => setQuery(e.target.value)} className="pl-10" autoFocus />
    </div>
    {!hasQ ? (
      <EmptyState icon={<PackagePlus className="h-10 w-10" />} title={t('skills.discover')} description={t('skills.discoverDescription')} />
    ) : isLoading ? <LoadingCards count={4} /> : isError ? <ErrorState onRetry={() => refetch()} /> : results?.length === 0 ? (
      <EmptyState icon={<Search className="h-10 w-10" />} title={t('skills.noResults')} description={t('skills.noResultsFor', { query: deferredQuery })} />
    ) : (
      <div className="grid gap-4">{results!.map(s => <MarketplaceCard key={s.slug} skill={s} isInstalling={mut.isPending} onInstall={(sl, v) => mut.mutate({ slug: sl, version: v })} />)}</div>
    )}
  </>)
}

// ─── Skill Card ───────────────────────────────────────────────

function SkillCard({ skill }: { skill: Skill }) {
  const { t } = useTranslation()
  const sd = STATUS_DISPLAY[skill.status]
  const isClaude = skill.format === 'claude_code'
  const hasMissing = skill.missing.bins.length > 0 || skill.missing.anyBins.length > 0 || skill.missing.env.length > 0 || skill.missing.config.length > 0

  return (
    <Card className="transition-shadow hover:shadow-md">
      <CardContent className="p-5 space-y-3">
        <div className="flex items-start justify-between gap-3">
          <div className="flex items-start gap-2 min-w-0">
            <span className="text-lg leading-none mt-0.5 shrink-0">{skill.emoji || '⚡'}</span>
            <div className="min-w-0">
              <h3 className="font-semibold text-base leading-tight">{skill.name}</h3>
              {skill.description && <p className="text-sm text-muted-foreground mt-0.5 line-clamp-2">{skill.description}</p>}
            </div>
          </div>
          <div className="flex items-center gap-2 shrink-0">
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
      </CardContent>
    </Card>
  )
}

// ─── Marketplace Card ─────────────────────────────────────────

function MarketplaceCard({ skill, isInstalling, onInstall }: { skill: ClawHubSearchResult; isInstalling: boolean; onInstall: (s: string, v?: string) => void }) {
  const { t } = useTranslation()
  const v = skill.version; const dn = skill.displayName || skill.slug
  const rt = useMemo(() => { if (!skill.updatedAt) return null; const d = Math.floor((Date.now() - skill.updatedAt) / 86_400_000); if (d === 0) return 'today'; if (d === 1) return '1d ago'; if (d < 30) return `${d}d ago`; const w = Math.floor(d / 7); if (w < 4) return `${w}w ago`; return `${Math.floor(d / 30)}mo ago` }, [skill.updatedAt])
  return (
    <Card className="transition-shadow hover:shadow-md">
      <CardContent className="p-5 space-y-3">
        <div className="flex items-start justify-between gap-3">
          <div className="flex items-start gap-2 min-w-0">
            <span className="text-lg leading-none mt-0.5 shrink-0">🔍</span>
            <div className="min-w-0">
              <h3 className="font-semibold text-base leading-tight">{dn}</h3>
              {skill.summary && <p className="text-sm text-muted-foreground mt-0.5 line-clamp-2">{skill.summary}</p>}
            </div>
          </div>
          <div className="flex items-center gap-2 shrink-0">
            <FormatBadge format="openclaw" />
            {v && <Badge variant="outline" className="text-xs font-mono">v{v}</Badge>}
            <Button size="sm" onClick={() => onInstall(skill.slug, v)} disabled={isInstalling} className="gap-1.5">{t('skills.install')}</Button>
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