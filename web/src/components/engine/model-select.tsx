import { Check, ChevronDown, Search, X } from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { cn } from '@/lib/utils'
import type { ModelInfo } from '@/types/engine'

// ─── Helpers ─────────────────────────────────────────────────

function formatContextWindow(tokens: number): string {
  if (tokens >= 1_000_000) return `${(tokens / 1_000_000).toFixed(1)}M`
  if (tokens >= 1000) return `${Math.round(tokens / 1000)}K`
  return String(tokens)
}

function formatCost(cost: number): string {
  if (cost === 0) return 'Free'
  if (cost < 0.01) return '<$0.01'
  return `$${cost.toFixed(2)}`
}

// ─── Component ───────────────────────────────────────────────

interface ModelSelectProps {
  models: ModelInfo[]
  value: string | null
  onValueChange: (modelId: string) => void
  /** Optional id → display name map. Falls back to the model.provider id. */
  providersById?: Map<string, string>
  className?: string
}

export function ModelSelect({
  models,
  value,
  onValueChange,
  providersById,
  className,
}: ModelSelectProps) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')
  const ref = useRef<HTMLDivElement>(null)
  const inputRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false)
    }
    if (open) document.addEventListener('mousedown', handleClickOutside)
    return () => document.removeEventListener('mousedown', handleClickOutside)
  }, [open])

  // Reset search query when closing; focus input when opening.
  useEffect(() => {
    if (open) {
      requestAnimationFrame(() => inputRef.current?.focus())
    } else {
      setQuery('')
    }
  }, [open])

  const selected = models.find((m) => m.id === value)

  // Filter by search query (name, id, or provider name). Case-insensitive.
  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase()
    if (!q) return null
    return models.filter((m) => {
      if (m.name.toLowerCase().includes(q)) return true
      if (m.id.toLowerCase().includes(q)) return true
      const providerName = providersById?.get(m.provider) ?? m.provider
      if (providerName.toLowerCase().includes(q)) return true
      return false
    })
  }, [models, query, providersById])

  // Group by provider, then split each provider's models into reasoning / standard.
  // Order providers by their first appearance in the model list to keep stable
  // ordering across renders.
  const grouped = models.reduce<Map<string, { reasoning: ModelInfo[]; standard: ModelInfo[] }>>(
    (acc, m) => {
      const bucket = acc.get(m.provider) ?? { reasoning: [], standard: [] }
      if (m.reasoning) bucket.reasoning.push(m)
      else bucket.standard.push(m)
      acc.set(m.provider, bucket)
      return acc
    },
    new Map(),
  )
  const providerOrder = Array.from(new Set(models.map((m) => m.provider)))
  const providerLabel = (id: string) => providersById?.get(id) ?? id
  const isFiltering = filtered !== null

  return (
    <div className={cn('relative', className)} ref={ref}>
      <button
        type="button"
        onClick={() => setOpen(!open)}
        disabled={models.length === 0}
        className="flex h-9 w-full items-center justify-between rounded-md border border-input bg-transparent px-3 py-2 text-sm shadow-sm ring-offset-background hover:bg-accent/50 focus:outline-none focus:ring-1 focus:ring-ring disabled:opacity-50 disabled:cursor-not-allowed"
      >
        <span className={selected ? '' : 'text-muted-foreground'}>
          {selected ? (
            <span className="flex items-center gap-1.5">
              {selected.reasoning && <span title={t('engine.supportsReasoning')}>✦</span>}
              {selected.input.includes('image') && (
                <span title={t('engine.supportsVision')}>👁</span>
              )}
              {selected.name}
            </span>
          ) : models.length === 0 ? (
            t('engine.noModelsAvailable')
          ) : (
            t('engine.selectModel')
          )}
        </span>
        <ChevronDown className="h-4 w-4 opacity-50" />
      </button>

      {open && models.length > 0 && (
        <div className="absolute z-50 mt-1 w-full overflow-hidden rounded-md border bg-popover text-popover-foreground shadow-md max-h-96 flex flex-col">
          {/* Search input */}
          <div className="flex items-center gap-2 border-b border-border px-3 py-2 shrink-0">
            <Search className="h-3.5 w-3.5 text-muted-foreground shrink-0" />
            <input
              ref={inputRef}
              type="text"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Escape') {
                  if (query) setQuery('')
                  else setOpen(false)
                }
              }}
              placeholder={t('engine.searchModel')}
              className="flex-1 bg-transparent text-sm outline-none placeholder:text-muted-foreground/60"
            />
            {query && (
              <button
                type="button"
                onClick={() => setQuery('')}
                className="text-muted-foreground hover:text-foreground shrink-0"
                title="Clear"
              >
                <X className="h-3.5 w-3.5" />
              </button>
            )}
          </div>

          {/* List */}
          <div className="overflow-y-auto flex-1">
            {isFiltering ? (
              filtered!.length === 0 ? (
                <div className="px-3 py-6 text-center text-xs text-muted-foreground">
                  {t('engine.noSearchResults', { query })}
                </div>
              ) : (
                <div>
                  {filtered!.map((m) => (
                    <ModelRow
                      key={m.id}
                      model={m}
                      selected={value === m.id}
                      onSelect={onValueChange}
                      onClose={() => setOpen(false)}
                    />
                  ))}
                </div>
              )
            ) : (
              providerOrder.map((providerId, idx) => {
                const bucket = grouped.get(providerId)
                if (!bucket) return null
                const { reasoning: rs, standard: ss } = bucket
                if (rs.length === 0 && ss.length === 0) return null
                return (
                  <div key={providerId}>
                    <div className="px-3 py-1.5 text-xs font-semibold text-muted-foreground bg-muted/50 sticky top-0">
                      {providerLabel(providerId)}
                    </div>
                    {rs.length > 0 && (
                      <div>
                        <div className="px-3 py-1 text-[11px] uppercase tracking-wide text-muted-foreground/80 flex items-center gap-1">
                          <span>✦</span> {t('engine.reasoningModels')}
                        </div>
                        {rs.map((m) => (
                          <ModelRow
                            key={m.id}
                            model={m}
                            selected={value === m.id}
                            onSelect={onValueChange}
                            onClose={() => setOpen(false)}
                          />
                        ))}
                      </div>
                    )}
                    {ss.length > 0 && (
                      <div>
                        {rs.length > 0 && (
                          <div className="px-3 py-1 text-[11px] uppercase tracking-wide text-muted-foreground/80">
                            {t('engine.standardModels')}
                          </div>
                        )}
                        {ss.map((m) => (
                          <ModelRow
                            key={m.id}
                            model={m}
                            selected={value === m.id}
                            onSelect={onValueChange}
                            onClose={() => setOpen(false)}
                          />
                        ))}
                      </div>
                    )}
                    {idx < providerOrder.length - 1 && <div className="h-px bg-border my-1" />}
                  </div>
                )
              })
            )}
          </div>
        </div>
      )}
    </div>
  )
}

// ─── Model row ───────────────────────────────────────────────

function ModelRow({
  model,
  selected,
  onSelect,
  onClose,
}: {
  model: ModelInfo
  selected: boolean
  onSelect: (id: string) => void
  onClose: () => void
}) {
  const { t } = useTranslation()

  return (
    <button
      type="button"
      className={cn(
        'relative flex w-full cursor-pointer items-start gap-2 px-3 py-2 text-sm outline-none hover:bg-accent hover:text-accent-foreground',
        selected && 'bg-accent text-accent-foreground',
      )}
      onClick={() => {
        onSelect(model.id)
        onClose()
      }}
    >
      {/* Selection indicator */}
      <span className="w-4 shrink-0 mt-0.5">{selected && <Check className="h-4 w-4" />}</span>

      {/* Model info */}
      <div className="flex-1 min-w-0 text-left">
        <div className="flex items-center gap-1.5">
          {model.reasoning && (
            <span className="text-warning text-xs" title={t('engine.supportsReasoning')}>
              ✦
            </span>
          )}
          {model.input.includes('image') && (
            <span className="text-info text-xs" title={t('engine.supportsVision')}>
              👁
            </span>
          )}
          <span className="font-medium truncate">{model.name}</span>
        </div>
        <div className="flex items-center gap-3 mt-0.5 text-xs text-muted-foreground">
          <span>
            {formatContextWindow(model.contextWindow)} {t('engine.ctx')}
          </span>
          <span>
            {t('engine.input')} {formatCost(model.costInput)}/M
          </span>
          <span>
            {t('engine.output')} {formatCost(model.costOutput)}/M
          </span>
        </div>
      </div>
    </button>
  )
}
