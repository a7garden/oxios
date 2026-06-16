import { Search, X } from 'lucide-react'
import { useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Checkbox } from '@/components/ui/checkbox'
import { Input } from '@/components/ui/input'
import { Separator } from '@/components/ui/separator'
import { useToolCatalog } from '@/hooks/use-tool-catalog'

interface AllowedToolsPickerProps {
  value: string[]
  onChange: (next: string[]) => void
  disabled?: boolean
}

const CATEGORY_ORDER = ['fs', 'exec', 'memory', 'comms', 'a2a', 'system'] as const

/** Human-readable category labels (fallback slugs). */
const CATEGORY_LABELS: Record<string, string> = {
  fs: 'categories.fs',
  exec: 'categories.exec',
  memory: 'categories.memory',
  comms: 'categories.comms',
  a2a: 'categories.a2a',
  system: 'categories.system',
}

const CATEGORY_ICONS: Record<string, string> = {
  fs: '📁',
  exec: '⚡',
  memory: '🧠',
  comms: '🌐',
  a2a: '🔗',
  system: '⚙️',
}

/**
 * Full tool checklist: shows ALL available tools from the catalog
 * with checkboxes. Checked = allowed, unchecked = not allowed.
 *
 * Replaces the old tag-input approach where users had to guess
 * which tools existed.
 */
export function AllowedToolsPicker({ value, onChange, disabled }: AllowedToolsPickerProps) {
  const { t } = useTranslation()
  const { data: catalog, isLoading } = useToolCatalog()
  const [search, setSearch] = useState('')

  // Group by category, filter by search.
  const grouped = useMemo(() => {
    if (!catalog) return []
    const filtered = search
      ? catalog.filter((tool) => tool.name.toLowerCase().includes(search.toLowerCase()))
      : catalog

    const groups = new Map<string, typeof filtered>()
    for (const cat of CATEGORY_ORDER) {
      const tools = filtered.filter((t) => t.category === cat)
      if (tools.length > 0) groups.set(cat, tools)
    }
    // Uncategorized
    const rest = filtered.filter(
      (t) => !CATEGORY_ORDER.includes(t.category as (typeof CATEGORY_ORDER)[number]),
    )
    if (rest.length > 0) groups.set('other', rest)

    return Array.from(groups.entries())
  }, [catalog, search])

  const selectedCount = value.length
  const totalCount = catalog?.length ?? 0

  const toggle = (tool: string) => {
    if (value.includes(tool)) {
      onChange(value.filter((v) => v !== tool))
    } else {
      onChange([...value, tool])
    }
  }

  const toggleAll = (tools: string[], checked: boolean) => {
    if (checked) {
      // Add all tools in this group that aren't already in the list.
      const toAdd = tools.filter((t) => !value.includes(t))
      onChange([...value, ...toAdd])
    } else {
      // Remove all tools in this group.
      onChange(value.filter((v) => !tools.includes(v)))
    }
  }

  if (isLoading) {
    return (
      <div className="text-sm text-muted-foreground py-4 text-center">{t('common.loading')}</div>
    )
  }

  return (
    <div className="space-y-3">
      {/* Search + count */}
      <div className="flex items-center gap-3">
        <div className="relative flex-1">
          <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-muted-foreground" />
          <Input
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder={t('settings.allowedToolsSearch')}
            className="h-8 pl-7 text-sm"
            disabled={disabled}
          />
          {search && (
            <button
              type="button"
              onClick={() => setSearch('')}
              className="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
            >
              <X className="h-3.5 w-3.5" />
            </button>
          )}
        </div>
        <span className="text-xs text-muted-foreground tabular-nums whitespace-nowrap">
          {selectedCount}/{totalCount}
        </span>
      </div>

      {/* Checklist */}
      <div className="rounded-md border bg-muted/20 max-h-[400px] overflow-y-auto">
        {grouped.length === 0 ? (
          <div className="p-4 text-sm text-muted-foreground text-center">
            {t('settings.allowedToolsNoMatch')}
          </div>
        ) : (
          grouped.map(([category, tools], gi) => (
            <div key={category}>
              {gi > 0 && <Separator />}
              {/* Category header with toggle all */}
              <div className="flex items-center gap-2 px-3 py-2 bg-muted/40 sticky top-0">
                <span className="text-xs">{CATEGORY_ICONS[category] ?? '📦'}</span>
                <span className="text-xs font-semibold text-foreground flex-1">
                  {t(CATEGORY_LABELS[category] ?? category, category)}
                </span>
                <span className="text-xs text-muted-foreground tabular-nums">
                  {tools.filter((t) => value.includes(t.name)).length}/{tools.length}
                </span>
                {!disabled && tools.length > 1 && (
                  <button
                    type="button"
                    className="text-xs text-muted-foreground hover:text-foreground ml-2"
                    onClick={() => {
                      const allChecked = tools.every((t) => value.includes(t.name))
                      toggleAll(
                        tools.map((t) => t.name),
                        !allChecked,
                      )
                    }}
                  >
                    {tools.every((t) => value.includes(t.name))
                      ? t('common.deselectAll')
                      : t('common.selectAll')}
                  </button>
                )}
              </div>
              {/* Tool rows */}
              {tools.map((tool) => {
                const checked = value.includes(tool.name)
                return (
                  <label
                    key={tool.name}
                    className={`flex items-center gap-3 px-3 py-1.5 text-sm cursor-pointer transition-colors
                      ${disabled ? 'opacity-50 cursor-not-allowed' : 'hover:bg-accent/50'}
                      ${checked ? 'bg-accent/20' : ''}`}
                  >
                    <Checkbox
                      checked={checked}
                      onCheckedChange={() => !disabled && toggle(tool.name)}
                      disabled={disabled}
                    />
                    <span className="font-mono text-xs font-medium">{tool.name}</span>
                    <span className="text-xs text-muted-foreground ml-auto">
                      {t(tool.description_key, tool.name)}
                    </span>
                  </label>
                )
              })}
            </div>
          ))
        )}
      </div>

      {/* Manual entry for non-catalog tools (MCP etc.) */}
      {!disabled && (
        <div className="text-xs text-muted-foreground">{t('settings.allowedToolsManualEntry')}</div>
      )}
    </div>
  )
}
