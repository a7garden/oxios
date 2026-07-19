import { Check,
  ChevronDown,
  Cpu,
  Eye,
  Info,
  Search,
  Settings2,
  Sparkles,
  Star,
  Tag,
  X,
} from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { useEngineConfig, useModels, useSetModel } from '@/hooks/use-engine'
import { cn } from '@/lib/utils'
import type { ModelInfo } from '@/types/engine'
import { ModelDetail } from '@/components/engine/model-detail'

// ─── Helpers ─────────────────────────────────────────────────

function shortModelId(id: string): string {
  if (!id) return ''
  return id.includes('/') ? (id.split('/').pop() ?? id) : id
}

function shortProvider(id: string): string {
  if (!id) return ''
  const seg = id.split('/')[0] ?? id
  if (seg.length <= 4) return seg.toUpperCase()
  return seg.charAt(0).toUpperCase() + seg.slice(1)
}

function formatContextWindow(tokens: number): string {
  if (tokens >= 1_000_000) return `${(tokens / 1_000_000).toFixed(1)}M`
  if (tokens >= 1000) return `${Math.round(tokens / 1000)}K`
  return String(tokens)
}

function formatCost(cost: number): string {
  if (cost === 0) return '0'
  if (cost < 0.01) return '<0.01'
  return cost.toFixed(2)
}

/** A role entry: name → model id. */
export interface RoleEntry {
  name: string
  model: string
}

// ─── Props ───────────────────────────────────────────────────

export interface ModelPickerProps {
  /** All models from all connected providers. */
  models: ModelInfo[]
  /** Currently active model id (null = no override → role/global default). */
  activeModelId: string | null
  setActiveModelId: (id: string | null) => void
  /** Global default model id from /api/engine/config. */
  defaultModelId: string | null
  /** Promote the current `activeModelId` into the global default. */
  setAsDefault: (modelId: string) => void
  /** RFC-032 roles (name → model id). Empty when none configured. */
  roles: RoleEntry[]
  /** Currently active role (null = no role). Mutually exclusive with activeModelId. */
  activeRole: string | null
  setActiveRole: (role: string | null) => void
}

// ─── Component ───────────────────────────────────────────────

/**
 * ModelPicker — the unified chat-input pill for model + role routing.
 *
 * Single entry point. Model override and role are mutually exclusive:
 * picking a model clears the active role, and vice versa. This avoids
 * the sync problems of having two separate pills.
 *
 * Trigger shows: provider dot + label + chevron.
 *   - role active  → role name + its model short id
 *   - model active → model short name
 *   - neither      → "Default" + default model short id
 *
 * Popover layout (single panel, scrollable):
 *   ┌─ search ─────────────────────────────────┐
 *   ├─ ⌘ default model row                     │
 *   ├─ Provider: anthropic                     │
 *   │   ✦ reasoning / standard                 │
 *   ├─ Provider: openai …                      │
 *   ├─ Roles (when configured)                 │
 *   │   # fast     → claude-haiku              │
 *   │   # careful  → sonnet-4.5                │
 *   ├─ (empty hint when no roles)              │
 *   └─ footer: [Set as default] ⌨ ↑↓ │
 */
export function ModelPicker({
  models,
  activeModelId,
  setActiveModelId,
  defaultModelId,
  setAsDefault,
  roles,
  activeRole,
  setActiveRole,
}: ModelPickerProps) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')
  const [focusIndex, setFocusIndex] = useState(0)
  const searchRef = useRef<HTMLInputElement>(null)
  const listRef = useRef<HTMLDivElement>(null)

  const selected = activeModelId ? (models.find((m) => m.id === activeModelId) ?? null) : null
  const defaultModel = defaultModelId ? (models.find((m) => m.id === defaultModelId) ?? null) : null

  // Resolve the active role entry and the model it routes to.
  const activeRoleEntry = activeRole ? (roles.find((r) => r.name === activeRole) ?? null) : null
  const roleModel = activeRoleEntry
    ? (models.find((m) => m.id === activeRoleEntry.model) ?? null)
    : null
  // The model actually being used right now: explicit override > role's model > default.
  const routingModel = selected ?? roleModel ?? defaultModel

  // ── Flattened, filterable list of rows for keyboard nav (↑↓). ──
  const flatRows = useMemo(() => {
    const q = query.trim().toLowerCase()
    const rows: {
      kind: 'default' | 'model' | 'role'
      model: ModelInfo | null
      role?: RoleEntry
    }[] = []

    // "Default model" row.
    if (
      defaultModel &&
      (!q || 'default'.includes(q) || shortModelId(defaultModel.id).toLowerCase().includes(q))
    ) {
      rows.push({ kind: 'default', model: defaultModel })
    }

    // Group models by provider, then reasoning/standard.
    const providerOrder: string[] = []
    const grouped = new Map<string, { reasoning: ModelInfo[]; standard: ModelInfo[] }>()
    for (const m of models) {
      const bucket = grouped.get(m.provider) ?? { reasoning: [], standard: [] }
      if (m.reasoning) bucket.reasoning.push(m)
      else bucket.standard.push(m)
      grouped.set(m.provider, bucket)
      if (!providerOrder.includes(m.provider)) providerOrder.push(m.provider)
    }

    for (const providerId of providerOrder) {
      const bucket = grouped.get(providerId)
      if (!bucket) continue
      const matches = (m: ModelInfo) =>
        !q ||
        m.name.toLowerCase().includes(q) ||
        m.id.toLowerCase().includes(q) ||
        providerId.toLowerCase().includes(q)
      for (const m of bucket.reasoning.filter(matches)) rows.push({ kind: 'model', model: m })
      for (const m of bucket.standard.filter(matches)) rows.push({ kind: 'model', model: m })
    }

    // Roles — searchable by role name or model id.
    for (const r of roles) {
      if (
        !q ||
        r.name.toLowerCase().includes(q) ||
        r.model.toLowerCase().includes(q) ||
        shortModelId(r.model).toLowerCase().includes(q)
      ) {
        rows.push({ kind: 'role', model: null, role: r })
      }
    }

    return rows
  }, [models, defaultModel, roles, query])

  // Reset keyboard focus when the row set changes.
  useEffect(() => {
    setFocusIndex((i) => Math.min(i, Math.max(0, flatRows.length - 1)))
  }, [flatRows.length, open])

  // Auto-focus search on open; reset query on close.
  useEffect(() => {
    if (open) {
      requestAnimationFrame(() => searchRef.current?.focus())
    } else {
      setQuery('')
      setFocusIndex(0)
    }
  }, [open])

  // Scroll focused row into view.
  useEffect(() => {
    if (!open || !listRef.current) return
    const el = listRef.current.querySelector<HTMLButtonElement>(`[data-row="${focusIndex}"]`)
    el?.scrollIntoView({ block: 'nearest' })
  }, [focusIndex, open])

  const hasNoModels = models.length === 0
  const hasNoMatches = !hasNoModels && flatRows.length === 0
  const isCurrentDefault = !!activeModelId && !!defaultModelId && activeModelId === defaultModelId

  const onPickRow = (row: (typeof flatRows)[number]) => {
    if (row.kind === 'default' || !row.model) {
      // "Default" clears every override.
      setActiveModelId(null)
      setActiveRole(null)
    } else if (row.kind === 'role' && row.role) {
      // Role and model are mutually exclusive.
      setActiveRole(row.role.name)
      setActiveModelId(null)
    } else {
      // Picking a specific model clears any active role.
      setActiveModelId(row.model.id)
      setActiveRole(null)
    }
    setOpen(false)
  }

  const onTriggerKey = (e: React.KeyboardEvent<HTMLButtonElement>) => {
    if (e.key === 'ArrowDown' || e.key === 'Enter' || e.key === ' ') {
      e.preventDefault()
      setOpen(true)
    }
  }

  const onSearchKey = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'ArrowDown') {
      e.preventDefault()
      setFocusIndex((i) => (flatRows.length === 0 ? 0 : (i + 1) % flatRows.length))
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      setFocusIndex((i) =>
        flatRows.length === 0 ? 0 : (i - 1 + flatRows.length) % flatRows.length,
      )
    } else if (e.key === 'Enter') {
      e.preventDefault()
      const row = flatRows[focusIndex]
      if (row) onPickRow(row)
    } else if (e.key === 'Escape') {
      if (query) {
        e.preventDefault()
        setQuery('')
      } else {
        setOpen(false)
      }
    }
  }

  // Trigger label: role name takes priority (it's the most specific intent).
  const triggerLabel = activeRoleEntry
    ? activeRoleEntry.name
    : routingModel
      ? shortModelId(routingModel.id)
      : t('chat.modelPicker.defaultLabel')
  const triggerProvider = activeRoleEntry
    ? roleModel
      ? shortProvider(roleModel.id.split('/')[0] ?? '')
      : null
    : routingModel
      ? shortProvider(routingModel.id.split('/')[0] ?? '')
      : null

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          onKeyDown={onTriggerKey}
          className={cn(
            'group inline-flex items-center gap-1.5 h-7 max-w-[260px] truncate rounded-full',
            'border border-border/60 bg-background/60 px-2.5 text-xs',
            'hover:bg-accent/50 hover:border-border transition-colors',
            'focus:outline-none focus-visible:ring-1 focus-visible:ring-ring',
            activeRoleEntry && 'border-primary/40 bg-primary/5',
          )}
          title={t('chat.modelPicker.triggerHint')}
          aria-label={t('chat.modelPicker.trigger')}
        >
          <span
            className={cn(
              'h-1.5 w-1.5 rounded-full shrink-0',
              activeRoleEntry
                ? 'bg-primary'
                : routingModel
                  ? 'bg-success'
                  : 'bg-muted-foreground/40',
            )}
            aria-hidden
          />
          {activeRoleEntry && <Tag className="h-3 w-3 shrink-0 text-primary" aria-hidden />}
          <span className="truncate font-medium">{triggerLabel}</span>
          {triggerProvider && (
            <span className="text-muted-foreground/70 text-2xs truncate hidden sm:inline">
              · {triggerProvider}
            </span>
          )}
          <ChevronDown className="h-3 w-3 text-muted-foreground/60 shrink-0 transition-transform group-data-[state=open]:rotate-180" />
        </button>
      </PopoverTrigger>

      <PopoverContent
        align="start"
        side="top"
        className="w-[min(28rem,calc(100vw-2rem))] p-0 overflow-hidden"
        onOpenAutoFocus={(e) => e.preventDefault()}
      >
        {/* ── Search ────────────────────────────────────────── */}
        <div className="flex items-center gap-2 border-b border-border px-3 h-9">
          <Search className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
          <input
            ref={searchRef}
            value={query}
            onChange={(e) => {
              setQuery(e.target.value)
              setFocusIndex(0)
            }}
            onKeyDown={onSearchKey}
            placeholder={t('chat.modelPicker.search')}
            className="flex-1 bg-transparent text-sm outline-none placeholder:text-muted-foreground/60"
          />
          {query && (
            <button
              type="button"
              onClick={() => setQuery('')}
              className="text-muted-foreground hover:text-foreground shrink-0"
              aria-label="Clear"
            >
              <X className="h-3.5 w-3.5" />
            </button>
          )}
        </div>

        {/* ── List ──────────────────────────────────────────── */}
        <div ref={listRef} className="max-h-[60vh] overflow-y-auto py-1">
          {hasNoModels ? (
            <EmptyState
              icon={<Cpu className="h-5 w-5" />}
              title={t('chat.modelPicker.noModels')}
              hint={t('chat.modelPicker.noModelsHint')}
            />
          ) : hasNoMatches ? (
            <EmptyState
              icon={<Search className="h-5 w-5" />}
              title={t('chat.modelPicker.noResults', { query }) as string}
            />
          ) : (
            <ListRows
              rows={flatRows}
              focusIndex={focusIndex}
              activeModelId={activeModelId}
              defaultModelId={defaultModelId}
              activeRole={activeRole}
              models={models}
              onHover={setFocusIndex}
              onPick={onPickRow}
            />
          )}
        </div>

        {/* ── Roles hint (when no roles configured) ─────────── */}
        {roles.length === 0 && !hasNoModels && !query && (
          <div className="border-t border-border px-3 py-1.5 bg-muted/20">
            <p className="text-2xs text-muted-foreground/70 flex items-center gap-1.5">
              <Tag className="h-3 w-3 shrink-0" />
              {t('chat.modelPicker.noRolesHint')}
            </p>
          </div>
        )}

        {/* ── Footer ────────────────────────────────────────── */}
        <div className="flex items-center justify-between gap-2 border-t border-border px-3 py-2 bg-muted/30">
          <button
            type="button"
            disabled={!activeModelId || isCurrentDefault}
            onClick={() => {
              if (activeModelId && !isCurrentDefault) setAsDefault(activeModelId)
            }}
            className={cn(
              'inline-flex items-center gap-1.5 text-2xs rounded-md px-1.5 py-1 transition-colors',
              'focus:outline-none focus-visible:ring-1 focus-visible:ring-ring',
              !activeModelId || isCurrentDefault
                ? 'text-muted-foreground/50 cursor-not-allowed'
                : 'text-foreground hover:bg-accent',
            )}
            title={
              !activeModelId
                ? t('chat.modelPicker.setAsDefaultHintNoModel')
                : isCurrentDefault
                  ? t('chat.modelPicker.currentDefault')
                  : t('chat.modelPicker.setAsDefault')
            }
          >
            <Star
              className={cn(
                'h-3 w-3',
                isCurrentDefault ? 'fill-primary text-primary' : 'text-muted-foreground',
              )}
            />
            {isCurrentDefault
              ? t('chat.modelPicker.currentDefault')
              : t('chat.modelPicker.setAsDefault')}
          </button>

          <div className="flex items-center gap-2 text-2xs text-muted-foreground/80">
            <span>
              {t('chat.modelPicker.kbd.navigate')} {t('chat.modelPicker.kbd.select')}{' '}
              {t('chat.modelPicker.kbd.close')}
            </span>
          </div>
        </div>
      </PopoverContent>
    </Popover>
  )
}

// ─── Internal: list rendering ────────────────────────────────

function ListRows({
  rows,
  focusIndex,
  activeModelId,
  defaultModelId,
  activeRole,
  models,
  onHover,
  onPick,
}: {
  rows: {
    kind: 'default' | 'model' | 'role'
    model: ModelInfo | null
    role?: RoleEntry
  }[]
  focusIndex: number
  activeModelId: string | null
  defaultModelId: string | null
  activeRole: string | null
  models: ModelInfo[]
  onHover: (i: number) => void
  onPick: (row: (typeof rows)[number]) => void
}) {
  const { t } = useTranslation()

  // Render model rows grouped by provider, then append role rows in a
  // separate section. We iterate the flat `rows` array but emit provider
  // headers between model groups, and a "Roles" header before the first
  // role row.
  let i = 0
  const out: React.ReactNode[] = []
  let sawFirstRole = false

  while (i < rows.length) {
    const row = rows[i]!

    if (row.kind === 'role') {
      // Emit a single "Roles" header before the first role row.
      if (!sawFirstRole) {
        sawFirstRole = true
        out.push(
          <div
            key="roles-header"
            className="px-2 mt-1.5 pb-1 text-2xs uppercase tracking-wider text-muted-foreground/70 font-semibold border-t border-border pt-2 flex items-center gap-1.5"
          >
            <Tag className="h-3 w-3" />
            {t('chat.modelPicker.rolesTitle')}
          </div>,
        )
      }
      const r = row.role!
      const roleModelInfo = models.find((m) => m.id === r.model) ?? null
      out.push(
        <RoleRow
          key={`role-${r.name}`}
          index={i}
          focused={i === focusIndex}
          role={r}
          roleModelInfo={roleModelInfo}
          selected={activeRole === r.name}
          onHover={onHover}
          onClick={() => onPick(row)}
        />,
      )
      i++
      continue
    }

    if (row.kind === 'default' || !row.model) {
      out.push(
        <RowButton
          key={`default-${i}`}
          index={i}
          focused={i === focusIndex}
          onHover={onHover}
          onClick={() => onPick(row)}
        >
          <div className="flex items-center gap-2 min-w-0 flex-1">
            <Sparkles className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
            <span className="text-xs font-medium truncate">
              {row.model ? shortModelId(row.model.id) : t('chat.modelPicker.defaultLabel')}
            </span>
            <span className="text-2xs text-muted-foreground/70 shrink-0">
              {t('chat.modelPicker.isDefault')}
            </span>
          </div>
          {activeModelId === null && activeRole === null && (
            <Check className="h-3.5 w-3.5 text-primary shrink-0" />
          )}
        </RowButton>,
      )
      i++
      continue
    }

    // Group consecutive model rows for the same provider.
    const providerId = row.model.provider
    const groupRows: { idx: number; model: ModelInfo }[] = []
    while (i < rows.length) {
      const r = rows[i]!
      if (r.kind === 'model' && r.model && r.model.provider === providerId) {
        groupRows.push({ idx: i, model: r.model })
        i++
      } else {
        break
      }
    }
    const hasReasoning = groupRows.some((g) => g.model.reasoning)
    const hasStandard = groupRows.some((g) => !g.model.reasoning)
    out.push(
      <div key={`prov-${providerId}-${groupRows[0]!.idx}`} className="px-1 pt-1.5">
        <div className="px-2 pb-1 text-2xs uppercase tracking-wider text-muted-foreground/70 font-semibold flex items-center gap-1.5">
          {shortProvider(providerId)}
          <span className="text-muted-foreground/40 font-normal normal-case tracking-normal">
            · {groupRows.length}
          </span>
        </div>
        {hasReasoning && (
          <SubHeader icon={<Sparkles className="h-3 w-3" />}>
            {t('chat.modelPicker.reasoning')}
          </SubHeader>
        )}
        {groupRows
          .filter((g) => g.model.reasoning)
          .map((g) => (
            <ModelRow
              key={g.model.id}
              index={g.idx}
              focused={g.idx === focusIndex}
              model={g.model}
              activeModelId={activeModelId}
              defaultModelId={defaultModelId}
              onHover={onHover}
              onClick={() => onPick(rows[g.idx]!)}
            />
          ))}
        {hasStandard && <SubHeader>{t('chat.modelPicker.standard')}</SubHeader>}
        {groupRows
          .filter((g) => !g.model.reasoning)
          .map((g) => (
            <ModelRow
              key={g.model.id}
              index={g.idx}
              focused={g.idx === focusIndex}
              model={g.model}
              activeModelId={activeModelId}
              defaultModelId={defaultModelId}
              onHover={onHover}
              onClick={() => onPick(rows[g.idx]!)}
            />
          ))}
      </div>,
    )
  }
  return <>{out}</>
}

function SubHeader({ icon, children }: { icon?: React.ReactNode; children: React.ReactNode }) {
  return (
    <div className="px-2 py-1 text-2xs text-muted-foreground/60 flex items-center gap-1">
      {icon}
      {children}
    </div>
  )
}

function RowButton({
  index,
  focused,
  onHover,
  onClick,
  children,
}: {
  index: number
  focused: boolean
  onHover: (i: number) => void
  onClick: () => void
  children: React.ReactNode
}) {
  return (
    <button
      type="button"
      data-row={index}
      onMouseEnter={() => onHover(index)}
      onClick={onClick}
      className={cn(
        'flex items-center gap-2 w-full px-3 py-1.5 text-left transition-colors',
        'focus:outline-none focus-visible:bg-accent',
        focused ? 'bg-accent' : 'hover:bg-accent/50',
      )}
    >
      {children}
    </button>
  )
}

function ModelRow({
  index,
  focused,
  model,
  activeModelId,
  defaultModelId,
  onHover,
  onClick,
}: {
  index: number
  focused: boolean
  model: ModelInfo
  activeModelId: string | null
  defaultModelId: string | null
  onHover: (i: number) => void
  onClick: () => void
}) {
  const { t } = useTranslation()
  const selected = model.id === activeModelId
  const isDefault = model.id === defaultModelId
  return (
    <RowButton index={index} focused={focused} onHover={onHover} onClick={onClick}>
      <div className="flex items-center gap-1.5 min-w-0 flex-1">
        {model.reasoning && (
          <span
            title={t('chat.modelPicker.supportsReasoning')}
            className="inline-flex shrink-0 text-warning"
          >
            <Sparkles className="h-3 w-3" />
          </span>
        )}
        {model.input.includes('image') && (
          <span
            title={t('chat.modelPicker.supportsVision')}
            className="inline-flex shrink-0 text-info"
          >
            <Eye className="h-3 w-3" />
          </span>
        )}
        <span className="text-xs font-medium truncate">{model.name}</span>
        <Popover>
          <PopoverTrigger asChild>
            <button
              type="button"
              onClick={(e) => e.stopPropagation()}
              className="inline-flex shrink-0 text-muted-foreground/40 hover:text-muted-foreground transition-colors"
              title="Model details"
            >
              <Info className="h-3 w-3" />
            </button>
          </PopoverTrigger>
          <PopoverContent side="right" align="start" className="w-72 p-4">
            <ModelDetail model={model} />
          </PopoverContent>
        </Popover>
        {isDefault && (
          <span className="text-2xs text-primary/80 font-medium shrink-0">
            · {t('chat.modelPicker.isDefault')}
          </span>
        )}
      </div>
      <div className="flex items-center gap-2 text-2xs text-muted-foreground/80 shrink-0 tabular-nums">
        <span>{formatContextWindow(model.contextWindow)}</span>
        <span className="text-muted-foreground/40">·</span>
        <span>
          ${formatCost(model.costInput)}/${formatCost(model.costOutput)}
        </span>
      </div>
      {selected && <Check className="h-3.5 w-3.5 text-primary shrink-0" />}
    </RowButton>
  )
}

function RoleRow({
  index,
  focused,
  role,
  roleModelInfo,
  selected,
  onHover,
  onClick,
}: {
  index: number
  focused: boolean
  role: RoleEntry
  roleModelInfo: ModelInfo | null
  selected: boolean
  onHover: (i: number) => void
  onClick: () => void
}) {
  const shortModel = shortModelId(role.model)
  const provider = role.model.includes('/') ? shortProvider(role.model.split('/')[0]!) : ''
  return (
    <RowButton index={index} focused={focused} onHover={onHover} onClick={onClick}>
      <Tag className="h-3 w-3 shrink-0 text-primary" />
      <span className="text-xs font-medium truncate">{role.name}</span>
      <span className="text-muted-foreground/50 text-2xs shrink-0">→</span>
      <span className="text-2xs text-muted-foreground truncate font-mono min-w-0">
        {provider && <span className="text-foreground/70 font-sans">{provider}/</span>}
        {shortModel}
      </span>
      {roleModelInfo?.reasoning && (
        <span className="text-warning text-2xs shrink-0" title="reasoning">
          ✦
        </span>
      )}
      {selected && <Check className="h-3.5 w-3.5 text-primary shrink-0 ml-auto" />}
    </RowButton>
  )
}

function EmptyState({
  icon,
  title,
  hint,
}: {
  icon: React.ReactNode
  title: string
  hint?: string
}) {
  return (
    <div className="flex flex-col items-center gap-1 px-4 py-8 text-center text-muted-foreground">
      <div className="opacity-50">{icon}</div>
      <p className="text-xs font-medium text-foreground/80">{title}</p>
      {hint && <p className="text-2xs text-muted-foreground/70 max-w-[18rem]">{hint}</p>}
    </div>
  )
}

// ─── Convenience wrapper ─────────────────────────────────────

export interface ModelPickerContainerProps {
  activeModelId: string | null
  setActiveModelId: (id: string | null) => void
  roles: RoleEntry[]
  activeRole: string | null
  setActiveRole: (role: string | null) => void
}

/**
 * Resolves `useModels(null) + useEngineConfig + useSetModel` and forwards
 * them as props to `ModelPicker`. Call sites should mount this; the bare
 * `ModelPicker` stays unit-testable.
 */
export function ModelPickerContainer(props: ModelPickerContainerProps) {
  const { data: models = [] } = useModels(null)
  const { data: engineConfig } = useEngineConfig()
  const setModel = useSetModel()

  return (
    <ModelPicker
      {...props}
      models={models}
      defaultModelId={engineConfig?.default_model ?? null}
      setAsDefault={(id) => setModel.mutate(id)}
    />
  )
}

// re-export for legacy imports
export { Settings2 }
