import { useQuery } from '@tanstack/react-query'
import { createFileRoute, useNavigate, useSearch } from '@tanstack/react-router'
import { Bot, Search, X } from 'lucide-react'
import { useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { type Column, DataTable } from '@/components/shared/data-table'
import { EmptyState } from '@/components/shared/empty-state'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingTable } from '@/components/shared/loading'
import { RefreshButton } from '@/components/shared/refresh-button'
import { StatusIndicator } from '@/components/shared/status-indicator'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Select } from '@/components/ui/select'
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { api } from '@/lib/api-client'
import type { AgentListItem, AgentListResponse } from '@/types/agent'

// ── Route ────────────────────────────────────────────────────────────

export const Route = createFileRoute('/agents/')({
  component: AgentsListPage,
  validateSearch: (search: Record<string, unknown>) => ({
    q: (search.q as string) || undefined,
    status: (search.status as string) || 'all',
    sort_by: (search.sort_by as string) || 'created_at',
    sort_dir: (search.sort_dir as string) || 'desc',
    page: Number(search.page) || 1,
    per_page: Number(search.per_page) || 50,
  }),
})

/** Default `/agents` search params — shared so other routes can link here
 *  with a type-safe, fully-populated search object. */
export const defaultAgentSearch = {
  q: undefined,
  status: 'all',
  sort_by: 'created_at',
  sort_dir: 'desc',
  page: 1,
  per_page: 50,
} as const

function buildQueryString(params: Record<string, string | number | undefined>) {
  const qs = new URLSearchParams()
  for (const [k, v] of Object.entries(params)) {
    if (v !== undefined && v !== '' && v !== 'all') {
      qs.set(k, String(v))
    }
  }
  return qs.toString()
}

// ── Stats Bar ────────────────────────────────────────────────────────

function StatsBar({ response }: { response: AgentListResponse | undefined }) {
  if (!response) return null

  const { stats, total } = response

  return (
    <div className="flex flex-wrap items-center gap-4 text-sm text-muted-foreground">
      <span className="font-semibold text-foreground">{total.toLocaleString()} agents</span>
      {stats.count_running > 0 && (
        <span className="text-green-500">{stats.count_running} running</span>
      )}
      {stats.count_failed > 0 && <span className="text-red-500">{stats.count_failed} failed</span>}
      <span>${stats.total_cost_usd.toFixed(2)} total</span>
      <span>{stats.total_tokens.toLocaleString()} tokens</span>
      {stats.avg_duration_secs > 0 && <span>avg {stats.avg_duration_secs.toFixed(1)}s</span>}
    </div>
  )
}

// ── Filter Chips ─────────────────────────────────────────────────────

function FilterChips({
  filters,
  onRemove,
  onClear,
}: {
  filters: Record<string, string>
  onRemove: (key: string) => void
  onClear: () => void
}) {
  const entries = Object.entries(filters).filter(([_, v]) => v && v !== 'all')
  if (entries.length === 0) return null

  return (
    <div className="flex flex-wrap items-center gap-2">
      {entries.map(([key, value]) => (
        <Badge key={key} variant="secondary" className="gap-1">
          {key.replace(/_/g, ' ')}: {value.length > 30 ? `${value.slice(0, 30)}…` : value}
          <button
            type="button"
            onClick={() => onRemove(key)}
            className="ml-1 hover:text-foreground"
          >
            <X className="h-3 w-3" />
          </button>
        </Badge>
      ))}
      <Button variant="ghost" size="sm" className="h-6 text-xs" onClick={onClear}>
        clear all
      </Button>
    </div>
  )
}

// ── Main Page ────────────────────────────────────────────────────────

function AgentsListPage() {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const search = useSearch({ from: Route.id })

  // Current filter values from URL
  const statusTab = (search.status as string) || 'all'
  const currentPage = Number(search.page) || 1
  const perPage = Number(search.per_page) || 50
  const searchQuery = (search.q as string) || ''
  const sortBy = (search.sort_by as string) || 'created_at'
  const sortDir = (search.sort_dir as string) || 'desc'

  // Build query params for API
  const queryString = useMemo(() => {
    return buildQueryString({
      q: searchQuery || undefined,
      status: statusTab === 'all' ? undefined : statusTab,
      page: currentPage,
      per_page: perPage,
      sort_by: sortBy,
      sort_dir: sortDir,
    })
  }, [searchQuery, statusTab, currentPage, perPage, sortBy, sortDir])

  const { data, isLoading, isError, refetch, isFetching } = useQuery({
    queryKey: ['agents', queryString],
    queryFn: () => api.get<AgentListResponse>(`/api/agents?${queryString}`),
    refetchInterval: statusTab === 'running' ? 3000 : 10000,
  })

  // Update URL param
  function setParam(key: string, value: string | number | undefined) {
    navigate({
      search: { ...search, [key]: value, page: key === 'page' ? value : 1 } as any,
    })
  }

  // Active filters for chips (excluding page/per_page/sort)
  const activeFilters: Record<string, string> = {}
  if (statusTab !== 'all') activeFilters.status = statusTab
  if (searchQuery) activeFilters.search = searchQuery

  const agents = data?.items ?? []

  // -- Columns --
  const columns: Column<AgentListItem>[] = [
    {
      header: t('agents.name'),
      mobilePriority: 'primary',
      accessor: (row: AgentListItem) => (
        <div className="flex items-center gap-2">
          <Bot className="h-4 w-4 shrink-0 text-muted-foreground" />
          <span className="font-medium truncate max-w-[280px]">{row.name}</span>
        </div>
      ),
    },
    {
      header: t('agents.status'),
      mobilePriority: 'secondary',
      accessor: (row: AgentListItem) => (
        <StatusIndicator status={row.status?.toLowerCase() ?? 'unknown'} />
      ),
    },
    {
      header: t('agents.cost', 'Cost'),
      mobilePriority: 'secondary',
      accessor: (row: AgentListItem) =>
        row.cost_usd > 0 ? (
          <span className="text-xs font-mono">${row.cost_usd.toFixed(4)}</span>
        ) : (
          <span className="text-muted-foreground">—</span>
        ),
    },
    {
      header: t('agents.duration', 'Duration'),
      mobilePriority: 'hidden',
      accessor: (row: AgentListItem) =>
        row.duration_secs != null ? (
          <span className="text-xs">{formatDuration(row.duration_secs)}</span>
        ) : row.status === 'running' ? (
          <span className="text-xs text-green-500">running…</span>
        ) : (
          <span className="text-muted-foreground">—</span>
        ),
    },
    {
      header: t('agents.created'),
      mobilePriority: 'hidden',
      accessor: (row: AgentListItem) => (
        <span className="text-xs">{new Date(row.created_at).toLocaleString()}</span>
      ),
    },
    {
      header: t('agents.session', 'Session'),
      mobilePriority: 'hidden',
      accessor: (row: AgentListItem) =>
        row.session_id ? (
          <span className="text-xs text-muted-foreground truncate max-w-[120px] block">
            {row.session_id.slice(0, 12)}…
          </span>
        ) : (
          <span className="text-muted-foreground">—</span>
        ),
    },
    {
      header: t('agents.tokens', 'Tokens'),
      mobilePriority: 'secondary',
      accessor: (row: AgentListItem) =>
        row.tokens_used > 0 ? (
          <span className="text-xs">{row.tokens_used.toLocaleString()}</span>
        ) : (
          <span className="text-muted-foreground">—</span>
        ),
    },
  ]

  return (
    <div className="space-y-4 animate-fade-in-up">
      {/* Header + stats */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">{t('agents.title')}</h1>
          <StatsBar response={data} />
        </div>
        <div className="flex items-center gap-2">
          <SortSelect
            value={sortBy}
            dir={sortDir}
            onChange={(by, dir) => {
              setParam('sort_by', by)
              setParam('sort_dir', dir)
            }}
          />
          <RefreshButton onClick={() => refetch()} isFetching={isFetching} />
        </div>
      </div>

      {/* Search */}
      <div className="relative">
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
        <Input
          className="pl-9"
          placeholder={t(
            'agents.searchPlaceholder',
            'Search agents by name, error, or tool output…',
          )}
          defaultValue={searchQuery}
          onChange={(e) => {
            const val = e.target.value
            // Debounce handled via URL update
            setTimeout(() => setParam('q', val || undefined), 300)
          }}
        />
      </div>

      {/* Filter chips */}
      <FilterChips
        filters={activeFilters}
        onRemove={(key) => setParam(key, key === 'status' ? 'all' : undefined)}
        onClear={() => {
          navigate({ search: { ...defaultAgentSearch } as any })
        }}
      />

      {/* Status tabs */}
      <Tabs value={statusTab} onValueChange={(v) => setParam('status', v === 'all' ? 'all' : v)}>
        <TabsList>
          <TabsTrigger value="all">
            {t('agents.all', 'All')}
            {data && (
              <Badge variant="outline" className="ml-1 text-xs">
                {data.total}
              </Badge>
            )}
          </TabsTrigger>
          <TabsTrigger value="running">
            {t('agents.running', 'Running')}
            {data?.stats.count_running ? (
              <Badge variant="outline" className="ml-1 text-xs">
                {data.stats.count_running}
              </Badge>
            ) : null}
          </TabsTrigger>
          <TabsTrigger value="completed">
            {t('agents.completed', 'Completed')}
            {data?.stats.count_completed ? (
              <Badge variant="outline" className="ml-1 text-xs">
                {data.stats.count_completed}
              </Badge>
            ) : null}
          </TabsTrigger>
          <TabsTrigger value="failed">
            {t('agents.failed', 'Failed')}
            {data?.stats.count_failed ? (
              <Badge variant="outline" className="ml-1 text-xs">
                {data.stats.count_failed}
              </Badge>
            ) : null}
          </TabsTrigger>
        </TabsList>
      </Tabs>

      {/* Table */}
      {isLoading ? (
        <LoadingTable rows={5} />
      ) : isError ? (
        <ErrorState onRetry={() => refetch()} />
      ) : agents.length === 0 ? (
        <EmptyState
          icon={<Bot className="h-10 w-10" />}
          title={t('agents.noAgents')}
          description={t('agents.noAgentsDescription')}
        />
      ) : (
        <>
          <DataTable
            columns={columns}
            data={agents}
            keyExtractor={(row) => row.id}
            onRowClick={(row) => navigate({ to: '/agents/$agentId', params: { agentId: row.id } })}
          />

          {/* Pagination */}
          {data && data.total_pages > 1 && (
            <div className="flex items-center justify-between pt-2">
              <span className="text-sm text-muted-foreground">
                {data.total} results · page {currentPage} of {data.total_pages}
              </span>
              <div className="flex items-center gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  disabled={currentPage <= 1}
                  onClick={() => setParam('page', currentPage - 1)}
                >
                  ← Previous
                </Button>
                <Button
                  variant="outline"
                  size="sm"
                  disabled={currentPage >= data.total_pages}
                  onClick={() => setParam('page', currentPage + 1)}
                >
                  Next →
                </Button>
              </div>
            </div>
          )}
        </>
      )}
    </div>
  )
}

// ── Sort Select ──────────────────────────────────────────────────────

function SortSelect({
  value,
  dir,
  onChange,
}: {
  value: string
  dir: string
  onChange: (by: string, dir: string) => void
}) {
  return (
    <Select
      value={`${dir === 'asc' ? '+' : '-'}${value}`}
      onValueChange={(v) => {
        const desc = v.startsWith('-')
        onChange(v.slice(1), desc ? 'desc' : 'asc')
      }}
      className="w-[160px] h-9 text-xs"
      options={[
        { label: 'Newest first', value: '-created_at' },
        { label: 'Oldest first', value: '+created_at' },
        { label: 'Most expensive', value: '-cost_usd' },
        { label: 'Least expensive', value: '+cost_usd' },
        { label: 'Longest duration', value: '-duration_secs' },
        { label: 'Shortest duration', value: '+duration_secs' },
        { label: 'Most tokens', value: '-tokens_total' },
        { label: 'Fewest tokens', value: '+tokens_total' },
        { label: 'Name Z→A', value: '-name' },
        { label: 'Name A→Z', value: '+name' },
      ]}
    />
  )
}

// ── Helpers ──────────────────────────────────────────────────────────

function formatDuration(secs: number): string {
  if (secs < 60) return `${secs}s`
  if (secs < 3600) return `${Math.floor(secs / 60)}m ${secs % 60}s`
  return `${Math.floor(secs / 3600)}h ${Math.floor((secs % 3600) / 60)}m`
}
