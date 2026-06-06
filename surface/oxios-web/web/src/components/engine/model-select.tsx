import { Check, ChevronDown } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
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
  className?: string
}

export function ModelSelect({ models, value, onValueChange, className }: ModelSelectProps) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const ref = useRef<HTMLDivElement>(null)

  useEffect(() => {
    function handleClickOutside(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false)
    }
    if (open) document.addEventListener('mousedown', handleClickOutside)
    return () => document.removeEventListener('mousedown', handleClickOutside)
  }, [open])

  const selected = models.find((m) => m.id === value)

  // Separate reasoning and non-reasoning models
  const reasoningModels = models.filter((m) => m.reasoning)
  const standardModels = models.filter((m) => !m.reasoning)

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
        <div className="absolute z-50 mt-1 w-full overflow-hidden rounded-md border bg-popover text-popover-foreground shadow-md max-h-96 overflow-y-auto">
          {/* Reasoning models */}
          {reasoningModels.length > 0 && (
            <div>
              <div className="px-3 py-1.5 text-xs font-semibold text-muted-foreground bg-muted/50 flex items-center gap-1">
                <span>✦</span> {t('engine.reasoningModels')}
              </div>
              {reasoningModels.map((m) => (
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

          {/* Standard models */}
          {standardModels.length > 0 && (
            <div>
              {reasoningModels.length > 0 && (
                <div className="px-3 py-1.5 text-xs font-semibold text-muted-foreground bg-muted/50 flex items-center gap-1">
                  {t('engine.standardModels')}
                </div>
              )}
              {standardModels.map((m) => (
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
