// Image generation config panel — wires the [image-gen] config section to the
// settings UI. Replaces the previous no-op stub (settings.tsx).
//
// Reads via useConfig(), writes via useSaveConfig() (deep-merge PATCH /api/config).
// The API key is NOT configured here — it is resolved at runtime from the same
// chain as the chat engine (env → [engine].api_key → ~/.oxios/auth.json).

import { Loader2 } from 'lucide-react'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'
import { useConfig, useSaveConfig } from '@/hooks/use-config'
import { ImageGenerationSettings } from './image-generation-settings'

interface ImageGenConfig {
  enabled?: boolean
  provider?: string
  base_url?: string
  default_model?: string
  default_num?: number
}

function asImageGen(config: unknown): ImageGenConfig {
  if (config && typeof config === 'object') {
    const ig = (config as Record<string, unknown>).image_gen
    if (ig && typeof ig === 'object') return ig as ImageGenConfig
  }
  return {}
}

export function ImageGenerationPanel() {
  const { t } = useTranslation()
  const { data: config } = useConfig()
  const saveConfig = useSaveConfig()
  const ig = asImageGen(config)

  const patch = (p: Partial<ImageGenConfig>) => {
    saveConfig.mutate({ image_gen: { ...ig, ...p } })
  }

  // Model text input: local state, patched on blur (avoid per-keystroke saves).
  const [model, setModel] = useState('')
  useEffect(() => {
    setModel(ig.default_model ?? '')
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ig.default_model])

  return (
    <div className="space-y-4">
      {/* Enable toggle */}
      <div className="flex items-center justify-between rounded-lg border bg-card p-4">
        <div className="pr-4">
          <Label className="text-sm font-medium">{t('settings.imageGeneration.enable')}</Label>
          <p className="text-xs text-muted-foreground">
            {t('settings.imageGeneration.enableHint')}
          </p>
        </div>
        <Switch checked={!!ig.enabled} onCheckedChange={(v) => patch({ enabled: v })} />
      </div>

      {/* Default model */}
      <div className="space-y-1.5 rounded-lg border bg-card p-4">
        <Label htmlFor="ig-model" className="text-sm font-medium">
          {t('settings.imageGeneration.defaultModel')}
        </Label>
        <Input
          id="ig-model"
          value={model}
          placeholder="gpt-image-1"
          onChange={(e) => setModel(e.target.value)}
          onBlur={() => {
            if (model !== (ig.default_model ?? '')) patch({ default_model: model })
          }}
        />
        <p className="text-xs text-muted-foreground">
          {t('settings.imageGeneration.defaultModelHint')}
        </p>
      </div>

      {/* Provider / base URL (advanced) */}
      <div className="grid gap-3 rounded-lg border bg-card p-4 sm:grid-cols-2">
        <div className="space-y-1.5">
          <Label htmlFor="ig-provider" className="text-sm font-medium">
            {t('settings.imageGeneration.provider')}
          </Label>
          <Input
            id="ig-provider"
            defaultValue={ig.provider ?? 'openai'}
            onBlur={(e) =>
              e.target.value !== (ig.provider ?? 'openai') && patch({ provider: e.target.value })
            }
          />
        </div>
        <div className="space-y-1.5">
          <Label htmlFor="ig-baseurl" className="text-sm font-medium">
            {t('settings.imageGeneration.baseUrl')}
          </Label>
          <Input
            id="ig-baseurl"
            defaultValue={ig.base_url ?? 'https://api.openai.com/v1'}
            onBlur={(e) =>
              e.target.value !== (ig.base_url ?? 'https://api.openai.com/v1') &&
              patch({ base_url: e.target.value })
            }
          />
        </div>
      </div>

      {/* Default image count (reuses the LobeHub-ported slider component) */}
      <ImageGenerationSettings
        defaultImageNum={ig.default_num ?? 1}
        onDefaultImageNumChange={(n) => patch({ default_num: n })}
      />

      {saveConfig.isPending && (
        <div className="flex items-center gap-2 text-xs text-muted-foreground">
          <Loader2 className="h-3 w-3 animate-spin" /> {t('settings.imageGeneration.saving')}
        </div>
      )}
    </div>
  )
}
