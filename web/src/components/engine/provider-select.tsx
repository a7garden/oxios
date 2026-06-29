import { Check, ChevronDown, Key, Search, X } from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { cn } from '@/lib/utils'
import type { ProviderCategory, ProviderInfo } from '@/types/engine'

// ─── Category definitions ────────────────────────────────────

const CATEGORY_ORDER: ProviderCategory[] = ['major', 'open', 'regional', 'local']

const CATEGORY_LABELS: Record<ProviderCategory, string> = {
  major: 'engine.majorProviders',
  open: 'engine.openSpecialty',
  regional: 'engine.regional',
  local: 'engine.localSelfHosted',
}

// ─── Component ───────────────────────────────────────────────

interface ProviderSelectProps {
  providers: ProviderInfo[]
  value: string | null
  onValueChange: (providerId: string) => void
  className?: string
}

export function ProviderSelect({
  providers,
  value,
  onValueChange,
  className,
}: ProviderSelectProps) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState('')
  const ref = useRef<HTMLDivElement>(null)
  const inputRef = useRef<HTMLInputElement>(null)

  // Close on outside click
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
      // Defer to next frame so the input is mounted.
      requestAnimationFrame(() => inputRef.current?.focus())
    } else {
      setQuery('')
    }
  }, [open])

  // Group providers by category
  const grouped = useMemo(() => {
    const map = new Map<ProviderCategory, ProviderInfo[]>()
    for (const p of providers) {
      const list = Array.isArray(map.get(p.category)) ? map.get(p.category)! : []
      list.push(p)
      map.set(p.category, list)
    }
    return map
  }, [providers])

  // Filter by search query (case-insensitive, name or id match)
  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase()
    if (!q) return null
    return providers.filter(
      (p) => p.name.toLowerCase().includes(q) || p.id.toLowerCase().includes(q),
    )
  }, [providers, query])

  const selected = providers.find((p) => p.id === value)
  const isFiltering = filtered !== null

  return (
    <div className={cn('relative', className)} ref={ref}>
      <button
        type="button"
        onClick={() => setOpen(!open)}
        className="flex h-9 w-full items-center justify-between rounded-md border border-input bg-transparent px-3 py-2 text-sm shadow-sm ring-offset-background hover:bg-accent/50 focus:outline-none focus:ring-1 focus:ring-ring"
      >
        <span className={selected ? '' : 'text-muted-foreground'}>
          {selected ? selected.name : t('engine.selectProvider')}
        </span>
        <ChevronDown className="h-4 w-4 opacity-50" />
      </button>

      {open && (
        <div className="absolute z-50 mt-1 w-full overflow-hidden rounded-md border bg-popover text-popover-foreground shadow-md max-h-80 flex flex-col">
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
              placeholder={t('engine.searchProvider')}
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
                  {filtered!.map((p) => (
                    <button
                      key={p.id}
                      type="button"
                      className={cn(
                        'relative flex w-full cursor-pointer items-center gap-2 px-3 py-2 text-sm outline-none hover:bg-accent hover:text-accent-foreground',
                        value === p.id && 'bg-accent text-accent-foreground',
                      )}
                      onClick={() => {
                        onValueChange(p.id)
                        setOpen(false)
                      }}
                    >
                      <span className="w-4 shrink-0">
                        {value === p.id && <Check className="h-4 w-4" />}
                      </span>
                      <span className="flex-1 text-left">{p.name}</span>
                      {p.modelCount > 0 && (
                        <span className="text-xs text-muted-foreground">{p.modelCount}</span>
                      )}
                      {p.hasKey ? (
                        <Key className="h-3.5 w-3.5 text-success shrink-0" />
                      ) : (
                        <Key className="h-3.5 w-3.5 text-muted-foreground/40 shrink-0" />
                      )}
                    </button>
                  ))}
                </div>
              )
            ) : (
              CATEGORY_ORDER.map((cat) => {
                const items = grouped.get(cat)
                if (!items || items.length === 0) return null
                return (
                  <div key={cat}>
                    <div className="px-3 py-1.5 text-xs font-semibold text-muted-foreground bg-muted/50">
                      {t(CATEGORY_LABELS[cat])}
                    </div>
                    {items.map((p) => (
                      <button
                        key={p.id}
                        type="button"
                        className={cn(
                          'relative flex w-full cursor-pointer items-center gap-2 px-3 py-2 text-sm outline-none hover:bg-accent hover:text-accent-foreground',
                          value === p.id && 'bg-accent text-accent-foreground',
                        )}
                        onClick={() => {
                          onValueChange(p.id)
                          setOpen(false)
                        }}
                      >
                        <span className="w-4 shrink-0">
                          {value === p.id && <Check className="h-4 w-4" />}
                        </span>
                        <span className="flex-1 text-left">{p.name}</span>
                        {p.modelCount > 0 && (
                          <span className="text-xs text-muted-foreground">{p.modelCount}</span>
                        )}
                        {p.hasKey ? (
                          <Key className="h-3.5 w-3.5 text-success shrink-0" />
                        ) : (
                          <Key className="h-3.5 w-3.5 text-muted-foreground/40 shrink-0" />
                        )}
                      </button>
                    ))}
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
