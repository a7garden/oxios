import { useQuery } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { Check, X, Zap } from 'lucide-react'
import { useMemo, useState } from 'react'
import { EmptyState } from '@/components/shared/empty-state'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { RefreshButton } from '@/components/shared/refresh-button'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { api } from '@/lib/api-client'
import { cn } from '@/lib/utils'
import type { Skill, SkillStatus } from '@/types'

export const Route = createFileRoute('/skills')({ component: SkillsPage })

type FilterTab = 'all' | SkillStatus

const STATUS_DISPLAY: Record<SkillStatus, { emoji: string; label: string; variant: 'success' | 'warning' | 'destructive' }> = {
  ready: { emoji: '🟢', label: 'ready', variant: 'success' },
  needs_setup: { emoji: '🟡', label: 'needs-setup', variant: 'warning' },
  disabled: { emoji: '🔴', label: 'disabled', variant: 'destructive' },
}

const SOURCE_VARIANT: Record<string, 'outline' | 'secondary' | 'default'> = {
  managed: 'outline',
  bundled: 'secondary',
  workspace: 'default',
}

function SkillsPage() {
  const [filter, setFilter] = useState<FilterTab>('all')
  const [search, setSearch] = useState('')

  const {
    data: skills,
    isLoading,
    isError,
    refetch,
    isFetching,
  } = useQuery({
    queryKey: ['skills'],
    queryFn: async () => {
      const res = await api.get<{ skills: Skill[] }>('/api/skills')
      return res.skills ?? []
    },
    refetchInterval: 30000,
  })

  const counts = useMemo(() => {
    const list = skills ?? []
    return {
      all: list.length,
      ready: list.filter((s) => s.status === 'ready').length,
      needs_setup: list.filter((s) => s.status === 'needs_setup').length,
      disabled: list.filter((s) => s.status === 'disabled').length,
    }
  }, [skills])

  const filtered = useMemo(() => {
    let list = skills ?? []
    if (filter !== 'all') {
      list = list.filter((s) => s.status === filter)
    }
    if (search.trim()) {
      const q = search.toLowerCase()
      list = list.filter(
        (s) =>
          s.name.toLowerCase().includes(q) || s.description.toLowerCase().includes(q),
      )
    }
    return list
  }, [skills, filter, search])

  if (isLoading) return <LoadingCards count={4} />
  if (isError) return <ErrorState onRetry={() => refetch()} />

  const allSkills = skills ?? []

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Skills</h1>
          <p className="text-muted-foreground">Manage agent skill definitions</p>
        </div>
        <RefreshButton onClick={() => refetch()} isFetching={isFetching} />
      </div>

      {/* Filters */}
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div className="inline-flex h-9 items-center rounded-lg bg-muted p-1 text-muted-foreground gap-0.5">
          {(
            [
              { key: 'all' as const, label: 'All' },
              { key: 'ready' as const, label: 'Ready' },
              { key: 'needs_setup' as const, label: 'Needs Setup' },
              { key: 'disabled' as const, label: 'Disabled' },
            ] as const
          ).map((tab) => (
            <button
              key={tab.key}
              onClick={() => setFilter(tab.key)}
              className={cn(
                'inline-flex items-center justify-center whitespace-nowrap rounded-md px-3 py-1 text-sm font-medium transition-all',
                filter === tab.key
                  ? 'bg-background text-foreground shadow'
                  : 'hover:bg-background/50',
              )}
            >
              {tab.label}
              <span className="ml-1.5 text-xs text-muted-foreground">
                {counts[tab.key === 'needs_setup' ? 'needs_setup' : tab.key]}
              </span>
            </button>
          ))}
        </div>
        <Input
          placeholder="Search skills..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="max-w-xs"
        />
      </div>

      {/* Content */}
      {filtered.length === 0 ? (
        <EmptyState
          icon={<Zap className="h-10 w-10" />}
          title={allSkills.length === 0 ? 'No skills' : 'No matching skills'}
          description={
            allSkills.length === 0
              ? 'Skills will appear here when they are loaded.'
              : 'Try adjusting your filter or search query.'
          }
        />
      ) : (
        <div className="grid gap-4">
          {filtered.map((skill) => (
            <SkillCard key={skill.name} skill={skill} />
          ))}
        </div>
      )}
    </div>
  )
}

function SkillCard({ skill }: { skill: Skill }) {
  const statusDisplay = STATUS_DISPLAY[skill.status]
  const hasMissing =
    skill.missing.bins.length > 0 ||
    skill.missing.anyBins.length > 0 ||
    skill.missing.env.length > 0 ||
    skill.missing.config.length > 0

  return (
    <Card className="transition-shadow hover:shadow-md">
      <CardContent className="p-5 space-y-3">
        {/* Header */}
        <div className="flex items-start justify-between gap-3">
          <div className="flex items-start gap-2 min-w-0">
            <span className="text-lg leading-none mt-0.5 shrink-0">
              {skill.emoji || '⚡'}
            </span>
            <div className="min-w-0">
              <h3 className="font-semibold text-base leading-tight">{skill.name}</h3>
              {skill.description && (
                <p className="text-sm text-muted-foreground mt-0.5 line-clamp-2">
                  {skill.description}
                </p>
              )}
            </div>
          </div>
          <div className="flex items-center gap-2 shrink-0">
            {skill.always && (
              <Badge variant="outline" className="text-xs">
                ⚫ always
              </Badge>
            )}
            <Badge variant={statusDisplay.variant} className="text-xs gap-1">
              <span>{statusDisplay.emoji}</span> {statusDisplay.label}
            </Badge>
          </div>
        </div>

        {/* Meta row */}
        <div className="flex items-center gap-2 text-xs text-muted-foreground">
          {skill.version && <span>v{skill.version}</span>}
          <Badge variant={SOURCE_VARIANT[skill.source] ?? 'outline'} className="text-xs">
            {skill.source}
          </Badge>
          {skill.author && <span>by {skill.author}</span>}
        </div>

        {/* Requirements */}
        {(skill.requirements.bins.length > 0 ||
          skill.requirements.anyBins.length > 0 ||
          skill.requirements.env.length > 0 ||
          skill.requirements.config.length > 0) && (
          <div className="space-y-1.5">
            <p className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
              requires
            </p>
            <div className="space-y-1 pl-2">
              {skill.requirements.bins.length > 0 && (
                <RequirementRow
                  label="bins"
                  items={skill.requirements.bins}
                  missing={skill.missing.bins}
                />
              )}
              {skill.requirements.anyBins.length > 0 && (
                <RequirementRow
                  label="any_bins"
                  items={skill.requirements.anyBins}
                  missing={skill.missing.anyBins}
                />
              )}
              {skill.requirements.env.length > 0 && (
                <RequirementRow
                  label="env"
                  items={skill.requirements.env}
                  missing={skill.missing.env}
                />
              )}
              {skill.requirements.config.length > 0 && (
                <RequirementRow
                  label="config"
                  items={skill.requirements.config}
                  missing={skill.missing.config}
                />
              )}
            </div>
          </div>
        )}

        {/* Install options */}
        {skill.install.length > 0 && (
          <div className="space-y-1.5">
            <p className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
              install
            </p>
            <div className="pl-2 space-y-1">
              {skill.install.map((spec, i) => (
                <div
                  key={`${spec.kind}-${i}`}
                  className="flex items-center gap-2 text-sm text-muted-foreground"
                >
                  <span className="text-xs font-mono bg-muted px-1.5 py-0.5 rounded">
                    {spec.kind}
                  </span>
                  <span>{spec.label ?? spec.bins.join(', ')}</span>
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Config checks */}
        {skill.config_checks.length > 0 && (
          <div className="space-y-1.5">
            <p className="text-xs font-medium text-muted-foreground uppercase tracking-wider">
              config
            </p>
            <div className="pl-2 flex flex-wrap gap-x-4 gap-y-1">
              {skill.config_checks.map((check) => (
                <span
                  key={check.path}
                  className={cn(
                    'text-xs flex items-center gap-1',
                    check.satisfied ? 'text-emerald-600 dark:text-emerald-400' : 'text-red-600 dark:text-red-400',
                  )}
                >
                  {check.satisfied ? (
                    <Check className="h-3 w-3" />
                  ) : (
                    <X className="h-3 w-3" />
                  )}
                  {check.path}
                </span>
              ))}
            </div>
          </div>
        )}

        {/* Missing summary (only when there are missing deps) */}
        {hasMissing && skill.status === 'needs_setup' && (
          <div className="rounded-md bg-amber-500/10 border border-amber-500/20 px-3 py-2">
            <p className="text-xs text-amber-700 dark:text-amber-400">
              Missing:{' '}
              {[
                ...skill.missing.bins.map((b) => `bin:${b}`),
                ...skill.missing.env.map((e) => `env:${e}`),
                ...skill.missing.config.map((c) => `config:${c}`),
                ...skill.missing.anyBins.map((b) => `any_bin:${b}`),
              ].join(', ')}
            </p>
          </div>
        )}
      </CardContent>
    </Card>
  )
}

function RequirementRow({
  label,
  items,
  missing,
}: {
  label: string
  items: string[]
  missing: string[]
}) {
  return (
    <div className="flex items-start gap-2 text-xs">
      <span className="text-muted-foreground w-16 shrink-0 pt-px">{label}</span>
      <div className="flex flex-wrap gap-x-3 gap-y-0.5">
        {items.map((item) => {
          const isMissing = missing.includes(item)
          return (
            <span
              key={item}
              className={cn(
                'flex items-center gap-1',
                isMissing
                  ? 'text-red-600 dark:text-red-400'
                  : 'text-emerald-600 dark:text-emerald-400',
              )}
            >
              {isMissing ? (
                <X className="h-3 w-3" />
              ) : (
                <Check className="h-3 w-3" />
              )}
              {item}
              {isMissing && (
                <span className="text-red-400 dark:text-red-500">(missing)</span>
              )}
            </span>
          )
        })}
      </div>
    </div>
  )
}
