// SystemAgentSettings — model assignment UI for system tasks
// Ported from LobeHub's ModelAssignmentsForm pattern.

import {
  Archive,
  Bot,
  Box,
  Brain,
  Image,
  Languages,
  type LucideIcon,
  MessageCircle,
  Sparkles,
  Tag,
  UserCircle,
  Wand2,
} from 'lucide-react'
import { useId } from 'react'
import { useTranslation } from 'react-i18next'
import { ModelSelect } from '@/components/engine/model-select'
import { Input } from '@/components/ui/input'
import { Switch } from '@/components/ui/switch'
import { useModels } from '@/hooks/use-engine'
import { cn } from '@/lib/utils'
import {
  type MemoryServiceModelConfig,
  SYSTEM_AGENT_METADATA,
  type SystemAgentConfig,
  type SystemAgentItem,
} from '@/types/system-agent'

const ICONS: Record<string, LucideIcon> = {
  Brain,
  Bot,
  Tag,
  Image,
  Languages,
  Archive,
  MessageCircle,
  Sparkles,
  Wand2,
  Box,
  UserCircle,
}

interface SystemAgentSettingsProps {
  systemAgents: SystemAgentConfig
  memoryModels: MemoryServiceModelConfig
  defaultModel?: string
  onChange: (config: {
    systemAgents?: Partial<SystemAgentConfig>
    memoryModels?: Partial<MemoryServiceModelConfig>
    defaultModel?: string
  }) => void
  className?: string
}

export function SystemAgentSettings({
  systemAgents,
  memoryModels,
  onChange,
  className,
}: SystemAgentSettingsProps) {
  const { t } = useTranslation()

  const groups = [
    {
      id: 'system' as const,
      title: t('settings.systemAgents.system'),
      items: SYSTEM_AGENT_METADATA.filter((m) => m.group === 'system'),
    },
    {
      id: 'memory' as const,
      title: t('settings.systemAgents.memory'),
      items: SYSTEM_AGENT_METADATA.filter((m) => m.group === 'memory'),
    },
    {
      id: 'optional' as const,
      title: t('settings.systemAgents.optional'),
      items: SYSTEM_AGENT_METADATA.filter((m) => m.group === 'optional'),
    },
  ]

  return (
    <div className={cn('space-y-6', className)}>
      {groups.map((group) => (
        <section key={group.id}>
          <h3 className="text-sm font-semibold text-foreground mb-3">{group.title}</h3>
          <div className="space-y-2">
            {group.items.map((meta) => {
              const config = getConfig(group.id, meta.key, systemAgents, memoryModels)
              return (
                <SystemAgentRow
                  key={meta.key}
                  meta={meta}
                  config={config}
                  onChange={(newConfig) => {
                    if (group.id === 'memory') {
                      onChange({ memoryModels: { [meta.key]: newConfig } })
                    } else {
                      onChange({ systemAgents: { [meta.key]: newConfig } })
                    }
                  }}
                />
              )
            })}
          </div>
        </section>
      ))}
    </div>
  )
}

interface SystemAgentRowProps {
  meta: (typeof SYSTEM_AGENT_METADATA)[number]
  config: SystemAgentItem
  onChange: (config: SystemAgentItem) => void
}

function SystemAgentRow({ meta, config, onChange }: SystemAgentRowProps) {
  const Icon = ICONS[meta.icon] ?? Brain
  const switchId = useId()
  const { data: models = [] } = useModels(null)

  return (
    <div className="flex items-start gap-3 rounded-lg border bg-card p-3">
      <div className="shrink-0 mt-0.5">
        <div className="w-8 h-8 rounded-md bg-muted flex items-center justify-center">
          <Icon className="w-4 h-4 text-muted-foreground" />
        </div>
      </div>
      <div className="flex-1 min-w-0">
        <div className="flex items-center gap-2 mb-0.5">
          <span className="text-sm font-medium">{meta.label}</span>
          <label htmlFor={switchId} className="ml-auto flex items-center cursor-pointer">
            <Switch
              id={switchId}
              checked={config.enabled !== false}
              onCheckedChange={(checked) => onChange({ ...config, enabled: checked })}
            />
          </label>
        </div>
        <p className="text-xs text-muted-foreground mb-2">{meta.description}</p>
        {config.enabled !== false && (
          <div className="flex items-center gap-2">
            <div className="flex-1 min-w-0">
              <ModelSelect
                models={models}
                value={config.model ?? null}
                onValueChange={(id: string) => onChange({ ...config, model: id })}
              />
            </div>
            {meta.supportsContextLimit && (
              <Input
                type="number"
                value={config.contextLimit ?? ''}
                onChange={(e) =>
                  onChange({
                    ...config,
                    contextLimit: e.target.value ? Number(e.target.value) : undefined,
                  })
                }
                placeholder="ctx limit"
                className="w-24 h-8 text-xs"
              />
            )}
          </div>
        )}
      </div>
    </div>
  )
}

function getConfig(
  group: 'system' | 'memory' | 'optional',
  key: string,
  systemAgents: SystemAgentConfig,
  memoryModels: MemoryServiceModelConfig,
): SystemAgentItem {
  if (group === 'memory') {
    return (memoryModels as Record<string, SystemAgentItem>)[key] ?? { enabled: true }
  }
  return (systemAgents as Record<string, SystemAgentItem>)[key] ?? { enabled: true }
}
