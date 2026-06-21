import { CircleX, Plus, RouteIcon, Zap } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Separator } from '@/components/ui/separator'
import { Switch } from '@/components/ui/switch'
import { useRoutingConfig, useSetRouting } from '@/hooks/use-engine'
import type { RoutingConfig } from '@/types/routing'

// ─── Translation keys ─────────────────────────────────────────
const tKeys = {
  routingTitle: 'settings.routing.title',
  routingDesc: 'settings.routing.desc',
  autoRouting: 'settings.routing.auto',
  autoRoutingDesc: 'settings.routing.autoDesc',
  costEfficient: 'settings.routing.costEfficient',
  costEfficientDesc: 'settings.routing.costEfficientDesc',
  fallbacks: 'settings.routing.fallbacks',
  fallbacksDesc: 'settings.routing.fallbacksDesc',
  excludedModels: 'settings.routing.excludedModels',
  excludedModelsDesc: 'settings.routing.excludedModelsDesc',
  addModel: 'settings.routing.addModel',
} as const

// ─── RoutingSection ───────────────────────────────────────────

export function RoutingSection() {
  const { t } = useTranslation()
  const { data: routing } = useRoutingConfig()
  const setRouting = useSetRouting()

  const update = (patch: Partial<RoutingConfig>) => setRouting.mutate(patch)

  if (!routing) return null

  return (
    <div className="space-y-6">
      <Separator />

      <div className="space-y-4">
        <div className="flex items-center gap-2">
          <RouteIcon className="h-4 w-4 text-muted-foreground" />
          <h3 className="text-lg font-semibold">{t(tKeys.routingTitle)}</h3>
        </div>
        <p className="text-sm text-muted-foreground">{t(tKeys.routingDesc)}</p>

        {/* Auto routing toggle */}
        <div className="flex items-start justify-between gap-4">
          <div className="space-y-0.5">
            <Label>{t(tKeys.autoRouting)}</Label>
            <p className="text-xs text-muted-foreground">{t(tKeys.autoRoutingDesc)}</p>
          </div>
          <Switch
            checked={routing.routingEnabled}
            onCheckedChange={(v) => update({ routingEnabled: v })}
          />
        </div>

        {/* Cost efficient toggle */}
        <div className="flex items-start justify-between gap-4">
          <div className="space-y-0.5">
            <Label
              className={`flex items-center gap-1 ${!routing.routingEnabled ? 'text-muted-foreground/50' : ''}`}
            >
              <Zap className="h-3.5 w-3.5 text-warning" />
              {t(tKeys.costEfficient)}
            </Label>
            <p className="text-xs text-muted-foreground">{t(tKeys.costEfficientDesc)}</p>
          </div>
          <Switch
            checked={routing.preferCostEfficient}
            onCheckedChange={(v) => update({ preferCostEfficient: v })}
            disabled={!routing.routingEnabled}
          />
        </div>

        <Separator />

        {/* Fallback models */}
        <FallbackModelsEditor
          models={routing.fallbackModels}
          onAdd={(m) => update({ fallbackModels: [...routing.fallbackModels, m] })}
          onRemove={(i) =>
            update({ fallbackModels: routing.fallbackModels.filter((_, idx) => idx !== i) })
          }
          disabled={!routing.routingEnabled}
        />

        {/* Excluded models */}
        <ExcludedModelsEditor
          models={routing.excludedModels}
          onAdd={(m) => update({ excludedModels: [...routing.excludedModels, m] })}
          onRemove={(m) =>
            update({ excludedModels: routing.excludedModels.filter((x) => x !== m) })
          }
          disabled={!routing.routingEnabled}
        />
      </div>
    </div>
  )
}

// ─── FallbackModelsEditor ────────────────────────────────────

function FallbackModelsEditor({
  models,
  onAdd,
  onRemove,
  disabled,
}: {
  models: string[]
  onAdd: (m: string) => void
  onRemove: (i: number) => void
  disabled?: boolean
}) {
  const { t } = useTranslation()
  const [newModel, setNewModel] = useState('')

  const handleAdd = () => {
    const trimmed = newModel.trim()
    if (!trimmed) return
    onAdd(trimmed)
    setNewModel('')
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      e.preventDefault()
      handleAdd()
    }
  }

  return (
    <div className="space-y-2">
      <Label>{t(tKeys.fallbacks)}</Label>
      <p className="text-xs text-muted-foreground">{t(tKeys.fallbacksDesc)}</p>

      <div className="space-y-2">
        {models.map((model, i) => (
          <div key={i} className="flex items-center gap-2">
            <span className="w-6 text-sm text-muted-foreground text-right shrink-0">{i + 1}.</span>
            <div className="flex-1 rounded-md border px-3 py-2 text-sm bg-muted/30">{model}</div>
            <Button
              variant="ghost"
              size="sm"
              className="h-8 w-8 p-0 shrink-0"
              onClick={() => onRemove(i)}
              title="Remove"
              disabled={disabled}
            >
              <CircleX className="h-4 w-4 text-muted-foreground" />
            </Button>
          </div>
        ))}

        <div className="flex items-center gap-2">
          <span className="w-6 shrink-0" />
          <Input
            value={newModel}
            onChange={(e) => setNewModel(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="provider/model-id"
            className="h-8 text-sm flex-1"
            disabled={disabled}
          />
          <Button
            variant="outline"
            size="sm"
            className="h-8 shrink-0"
            onClick={handleAdd}
            disabled={!newModel.trim() || disabled}
          >
            <Plus className="h-3.5 w-3.5 mr-1" />
            {t(tKeys.addModel)}
          </Button>
        </div>
      </div>
    </div>
  )
}

// ─── ExcludedModelsEditor ─────────────────────────────────────

function ExcludedModelsEditor({
  models,
  onAdd,
  onRemove,
  disabled,
}: {
  models: string[]
  onAdd: (m: string) => void
  onRemove: (m: string) => void
  disabled?: boolean
}) {
  const { t } = useTranslation()
  const [newModel, setNewModel] = useState('')

  const handleAdd = () => {
    const trimmed = newModel.trim()
    if (!trimmed) return
    onAdd(trimmed)
    setNewModel('')
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      e.preventDefault()
      handleAdd()
    }
  }

  return (
    <div className="space-y-2">
      <Label>{t(tKeys.excludedModels)}</Label>
      <p className="text-xs text-muted-foreground">{t(tKeys.excludedModelsDesc)}</p>

      <div className="flex flex-wrap gap-2">
        {models.map((model) => (
          <span
            key={model}
            className="inline-flex items-center gap-1 rounded-full bg-muted px-3 py-1 text-sm"
          >
            <span className="text-xs text-muted-foreground">🚫</span>
            {model}
            <button
              type="button"
              className={`ml-1 ${disabled ? 'text-muted-foreground/30 cursor-not-allowed' : 'text-muted-foreground hover:text-foreground'}`}
              onClick={() => !disabled && onRemove(model)}
              disabled={disabled}
            >
              <CircleX className="h-3 w-3" />
            </button>
          </span>
        ))}

        <div className="flex items-center gap-1">
          <Input
            value={newModel}
            onChange={(e) => setNewModel(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="provider/model-id"
            className="h-7 w-48 text-xs"
            disabled={disabled}
          />
          <Button
            variant="ghost"
            size="sm"
            className="h-7 px-2"
            onClick={handleAdd}
            disabled={!newModel.trim() || disabled}
          >
            <Plus className="h-3.5 w-3.5" />
          </Button>
        </div>
      </div>
    </div>
  )
}
