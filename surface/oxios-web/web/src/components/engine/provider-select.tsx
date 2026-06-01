import { Check, ChevronDown, Key } from 'lucide-react'
import { useState, useRef, useEffect, useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import type { ProviderInfo, ProviderCategory } from '@/types/engine'
import { cn } from '@/lib/utils'

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
  const ref = useRef<HTMLDivElement>(null)

  // Close on outside click
  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false)
    }
    if (open) document.addEventListener('mousedown', handleClickOutside)
    return () => document.removeEventListener('mousedown', handleClickOutside)
  }, [open])

  // Group providers by category
  const grouped = useMemo(() => {
    const map = new Map<ProviderCategory, ProviderInfo[]>()
    for (const p of providers) {
      const list = map.get(p.category) ?? []
      list.push(p)
      map.set(p.category, list)
    }
    return map
  }, [providers])

  const selected = providers.find((p) => p.id === value)

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
        <div className="absolute z-50 mt-1 w-full overflow-hidden rounded-md border bg-popover text-popover-foreground shadow-md max-h-80 overflow-y-auto">
          {CATEGORY_ORDER.map((cat) => {
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
                    {/* Selection indicator */}
                    <span className="w-4 shrink-0">
                      {value === p.id && <Check className="h-4 w-4" />}
                    </span>

                    {/* Provider name */}
                    <span className="flex-1 text-left">{p.name}</span>

                    {/* Model count badge */}
                    {p.modelCount > 0 && (
                      <span className="text-xs text-muted-foreground">
                        {p.modelCount}
                      </span>
                    )}

                    {/* API key indicator */}
                    {p.hasKey ? (
                      <Key className="h-3.5 w-3.5 text-emerald-500 shrink-0" />
                    ) : (
                      <Key className="h-3.5 w-3.5 text-muted-foreground/40 shrink-0" />
                    )}
                  </button>
                ))}
              </div>
            )
          })}
        </div>
      )}
    </div>
  )
}