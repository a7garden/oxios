// model-selector — compact dropdown for picking the model used by the
// coding agent session. Fetches available models from GET /api/engine/models
// via the shared useModels hook, with a small fallback when the endpoint
// isn't reachable.
//
// Sits inside the agent input footer next to the send button. Uses the
// project's DropdownMenu primitives + semantic Tailwind tokens.

import { Check, ChevronDown, Cpu, Loader2 } from 'lucide-react'
import { useMemo, useState } from 'react'
import { useModels } from '@/hooks/use-engine'
import type { ModelInfo } from '@/types/engine'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { cn } from '@/lib/utils'

export interface ModelSelectorProps {
  /** Currently selected model id (e.g. "anthropic/claude-sonnet-4"). */
  value: string | null
  /** Called when the user picks a different model. */
  onChange: (modelId: string) => void
  /** Optional className for the trigger. */
  className?: string
  /** Disable the trigger (e.g. while a request is in flight). */
  disabled?: boolean
}

/** Strip the "provider/" prefix from a model id for display. */
const modelShortName = (modelId: string) => {
  const slash = modelId.lastIndexOf('/')
  return slash >= 0 ? modelId.slice(slash + 1) : modelId
}

/** Provider bucket for grouping — derived from the model id prefix. */
const providerFromId = (modelId: string) => {
  const slash = modelId.indexOf('/')
  return slash > 0 ? modelId.slice(0, slash) : 'default'
}

/**
 * ModelSelector — small button-trigger dropdown listing the available
 * models. Grouped by provider so long lists stay scannable. The trigger
 * label collapses to the model short-name when the menu is closed.
 */
export function ModelSelector({
  value,
  onChange,
  className,
  disabled = false,
}: ModelSelectorProps) {
  // null provider → fetch models from every connected provider (RFC-032).
  const { data: models, isLoading, isError } = useModels(null)
  const [open, setOpen] = useState(false)

  // Group models by provider for the menu sections.
  const grouped = useMemo(() => {
    const map = new Map<string, ModelInfo[]>()
    for (const m of models ?? []) {
      const key = providerFromId(m.id)
      const list = map.get(key) ?? []
      list.push(m)
      map.set(key, list)
    }
    // Stable ordering: sort providers alphabetically, models within by id.
    return Array.from(map.entries())
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([provider, list]) => [provider, [...list].sort((a, b) => a.id.localeCompare(b.id))] as const)
  }, [models])

  const triggerLabel = value ? modelShortName(value) : 'Select model'

  return (
    <DropdownMenu open={open} onOpenChange={setOpen}>
      <DropdownMenuTrigger asChild>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          disabled={disabled || isLoading}
          className={cn(
            'h-7 gap-1.5 px-2 text-xs font-normal text-muted-foreground hover:text-foreground',
            className,
          )}
          aria-label="Select model"
        >
          {isLoading ? (
            <Loader2 className="size-3.5 animate-spin" />
          ) : (
            <Cpu className="size-3.5" />
          )}
          <span className="max-w-[160px] truncate">{triggerLabel}</span>
          <ChevronDown className="size-3 opacity-60" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="start" className="w-72 max-h-80 overflow-y-auto">
        <DropdownMenuLabel className="text-xs font-normal text-muted-foreground">
          {isError ? 'Model list unavailable' : 'Available models'}
        </DropdownMenuLabel>
        <DropdownMenuSeparator />
        {grouped.length === 0 ? (
          <div className="px-2 py-3 text-xs text-muted-foreground">
            {isLoading ? 'Loading models…' : 'No models available.'}
          </div>
        ) : (
          grouped.map(([provider, list]) => (
            <div key={provider}>
              <DropdownMenuLabel className="px-2 py-1 text-[10px] uppercase tracking-wider text-muted-foreground">
                {provider}
              </DropdownMenuLabel>
              {list.map((m) => {
                const selected = m.id === value
                return (
                  <DropdownMenuItem
                    key={m.id}
                    onSelect={() => {
                      onChange(m.id)
                      setOpen(false)
                    }}
                    className="flex items-center gap-2"
                  >
                    <span className="flex-1 truncate">
                      <span className="block text-sm">{m.name || modelShortName(m.id)}</span>
                      {m.name && m.name !== modelShortName(m.id) ? (
                        <span className="block text-[10px] text-muted-foreground truncate">
                          {modelShortName(m.id)}
                        </span>
                      ) : null}
                    </span>
                    {selected ? <Check className="size-3.5 text-primary" /> : null}
                  </DropdownMenuItem>
                )
              })}
            </div>
          ))
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
