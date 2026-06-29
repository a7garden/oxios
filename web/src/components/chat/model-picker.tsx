import { useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import {
  Check,
  ChevronDown,
  Cpu,
  Eye,
  Search,
  Settings2,
  Sparkles,
  Star,
  X,
} from 'lucide-react'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { useEngineConfig, useModels, useSetModel } from '@/hooks/use-engine'
import type { ModelInfo } from '@/types/engine'
import { cn } from '@/lib/utils'

// ─── Helpers ─────────────────────────────────────────────────

function shortModelId(id: string): string {
  if (!id) return ''
  return id.includes('/') ? (id.split('/').pop() ?? id) : id
}

function shortProvider(id: string): string {
  if (!id) return ''
  // 'anthropic' → 'Anthropic', keep acronyms uppercase, cap length.
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

// ─── Props ───────────────────────────────────────────────────

export interface ModelPickerProps {
  /** All models from all connected providers. */
  models: ModelInfo[]
  /** Currently active model id (null = no override → global default). */
  activeModelId: string | null
  setActiveModelId: (id: string | null) => void
  /** Global default model id from /api/engine/config. */
  defaultModelId: string | null
  /** Promote the current `activeModelId` into the global default. */
  setAsDefault: (modelId: string) => void
}

// ─── Component ───────────────────────────────────────────────

/**
 * ModelPicker — compact trigger pill in the chat input bottom bar.
 *
 * The popover surfaces only model routing concerns. Role routing
 * (RFC-032) lives in `RolePill`; this picker must not duplicate it.
 *
 * Trigger shows: provider dot + model short name + chevron.
 * The active model is always the model that will be used; null falls back
 * to the global default (also surfaced in the trigger when no override).
 *
 * Popover layout (single panel, scrollable):
 *   ┌─ search ─────────────────────────────────┐
 *   ├─ ⌘ default model row (or "current")     │
 *   ├─ Provider: anthropic                     │
 *   │   ✦ reasoning                           │
 *   │      • Claude Sonnet 4.5    [1M]  $/$   │
 *   │   standard                              │
 *   │      • Claude Haiku 3.5     [200K] $/$  │
 *   ├─ Provider: openai                        │
 *   │   …                                     │
 *   └─ footer: [Set as default] ⌨ ↑↓ │
 */
export function ModelPicker({
  models,
  activeModelId,
  setActiveModelId,
  defaultModelId,
  setAsDefault,
}: ModelPickerProps) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')
  const [focusIndex, setFocusIndex] = useState(0)
  const searchRef = useRef<HTMLInputElement>(null)
  const listRef = useRef<HTMLDivElement>(null)

  const selected = activeModelId
    ? (models.find((m) => m.id === activeModelId) ?? null)
    : null
  const defaultModel = defaultModelId
    ? (models.find((m) => m.id === defaultModelId) ?? null)
    : null
  // The "currently routing to" model: explicit override > global default > first available.
  const routingModel = selected ?? defaultModel

  // ── Flattened, filterable list of (model, flatIndex) rows. ──
  // We precompute this so keyboard nav (↑↓) maps to a single integer.
  const flatRows = useMemo(() => {
    const q = query.trim().toLowerCase()
    const rows: { kind: 'default' | 'model'; model: ModelInfo | null }[] = []

    // "Default model" row — always present when a default exists; the user
    // can pick it to clear their override.
    if (defaultModel && (!q || 'default'.includes(q) || shortModelId(defaultModel.id).toLowerCase().includes(q))) {
      rows.push({ kind: 'default', model: defaultModel })
    }

    // Group by provider, then by reasoning/standard.
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
      const reasoning = bucket.reasoning.filter(matches)
      const standard = bucket.standard.filter(matches)
      if (reasoning.length === 0 && standard.length === 0) continue
      for (const m of reasoning) rows.push({ kind: 'model', model: m })
      for (const m of standard) rows.push({ kind: 'model', model: m })
    }
    return rows
  }, [models, defaultModel, query])

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
  const isCurrentDefault =
    !!activeModelId && !!defaultModelId && activeModelId === defaultModelId

  const onPickRow = (row: { kind: 'default' | 'model'; model: ModelInfo | null }) => {
    if (row.kind === 'default' || !row.model) {
      setActiveModelId(null) // clear override → fall back to global default
    } else {
      setActiveModelId(row.model.id)
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
          )}
          title={t('chat.modelPicker.triggerHint', 'Choose a model for the next message')}
          aria-label={t('chat.modelPicker.trigger', 'Model')}
        >
          <span
            className={cn(
              'h-1.5 w-1.5 rounded-full shrink-0',
              routingModel ? 'bg-success' : 'bg-muted-foreground/40',
            )}
            aria-hidden
          />
          <span className="truncate font-medium">
            {routingModel ? shortModelId(routingModel.id) : t('chat.modelPicker.defaultLabel', 'Default model')}
          </span>
          {routingModel && (
            <span className="text-muted-foreground/70 text-2xs truncate hidden sm:inline">
              · {shortProvider(routingModel.id.split('/')[0] ?? '')}
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
            placeholder={t('chat.modelPicker.search', 'Search models…')}
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
              title={t('chat.modelPicker.noModels', 'No models available')}
              hint={t(
                'chat.modelPicker.noModelsHint',
                'Connect a provider in Settings to add models.',
              )}
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
              query={query}
              onHover={setFocusIndex}
              onPick={onPickRow}
            />
          )}
        </div>

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
                ? t('chat.modelPicker.setAsDefaultHintNoModel', 'Pick a model first')
                : isCurrentDefault
                  ? t('chat.modelPicker.currentDefault', 'Current default')
                  : t('chat.modelPicker.setAsDefault', 'Set as default')
            }
          >
            <Star
              className={cn(
                'h-3 w-3',
                isCurrentDefault ? 'fill-primary text-primary' : 'text-muted-foreground',
              )}
            />
            {isCurrentDefault
              ? t('chat.modelPicker.currentDefault', 'Current default')
              : t('chat.modelPicker.setAsDefault', 'Set as default')}
          </button>

          <div className="flex items-center gap-2 text-2xs text-muted-foreground/80">
            <span>
              {t('chat.modelPicker.kbd.navigate', '↑↓')} {t('chat.modelPicker.kbd.select', '↵')} {t('chat.modelPicker.kbd.close', 'esc')}
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
  query,
  onHover,
  onPick,
}: {
  rows: { kind: 'default' | 'model'; model: ModelInfo | null }[]
  focusIndex: number
  activeModelId: string | null
  defaultModelId: string | null
  query: string
  onHover: (i: number) => void
  onPick: (row: { kind: 'default' | 'model'; model: ModelInfo | null }) => void
}) {
  const { t } = useTranslation()

  // Group consecutive "model" rows by provider for headers. Each group:
  //   - if all reasoning, show one "Reasoning" subheader
  //   - if all standard, show one "Standard" subheader
  //   - if mixed, show both with a divider
  let i = 0
  const out: React.ReactNode[] = []
  while (i < rows.length) {
    const row = rows[i]!
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
              {row.model ? shortModelId(row.model.id) : t('chat.modelPicker.defaultLabel', 'Default model')}
            </span>
            <span className="text-2xs text-muted-foreground/70 shrink-0">
              {t('chat.modelPicker.isDefault', 'Default')}
            </span>
          </div>
          {activeModelId === null && <Check className="h-3.5 w-3.5 text-primary shrink-0" />}
        </RowButton>,
      )
      i++
      continue
    }
    // Group: collect all consecutive model rows for the same provider.
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
            {t('chat.modelPicker.reasoning', 'Reasoning')}
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
              query={query}
              onHover={onHover}
              onClick={() => onPick({ kind: 'model', model: g.model })}
            />
          ))}
        {hasStandard && (
          <SubHeader>
            {t('chat.modelPicker.standard', 'Standard')}
          </SubHeader>
        )}
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
              query={query}
              onHover={onHover}
              onClick={() => onPick({ kind: 'model', model: g.model })}
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
  query: string
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
            title={t('chat.modelPicker.supportsReasoning', 'Supports reasoning')}
            className="inline-flex shrink-0 text-warning"
          >
            <Sparkles className="h-3 w-3" />
          </span>
        )}
        {model.input.includes('image') && (
          <span
            title={t('chat.modelPicker.supportsVision', 'Supports vision')}
            className="inline-flex shrink-0 text-info"
          >
            <Eye className="h-3 w-3" />
          </span>
        )}
        <span className="text-xs font-medium truncate">{model.name}</span>
        {isDefault && (
          <span className="text-2xs text-primary/80 font-medium shrink-0">
            · {t('chat.modelPicker.isDefault', 'Default')}
          </span>
        )}
      </div>
      <div className="flex items-center gap-2 text-2xs text-muted-foreground/80 shrink-0 tabular-nums">
        <span>{formatContextWindow(model.contextWindow)}</span>
        <span className="text-muted-foreground/40">·</span>
        <span>${formatCost(model.costInput)}/${formatCost(model.costOutput)}</span>
      </div>
      {selected && <Check className="h-3.5 w-3.5 text-primary shrink-0" />}
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
