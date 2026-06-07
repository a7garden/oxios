import { useQueryClient } from '@tanstack/react-query'
import { createFileRoute, Link, useSearch } from '@tanstack/react-router'
import {
  Bot,
  Brain,
  Cpu,
  Database,
  Eye,
  Globe,
  MessageSquare,
  Monitor,
  Save,
  Send,
  Server,
  Shield,
  Sparkles,
  Terminal,
  Timer,
  Zap,
} from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { ApiKeyInput } from '@/components/engine/api-key-input'
import { ModelSelect } from '@/components/engine/model-select'
import { ProviderOptionsPanel } from '@/components/engine/provider-options'
import { ProviderSelect } from '@/components/engine/provider-select'
import { RoutingSection } from '@/components/engine/routing-section'
import type { SubNavGroup } from '@/components/layout/settings-layout'
import { SettingsLayout } from '@/components/layout/settings-layout'
import { ChannelsSection } from '@/components/settings/channels-section'
import { DiffPreview } from '@/components/settings/diff-preview'
import {
  NEW_SECTIONS,
  type SettingsFieldDef,
  type SettingsSectionDef,
} from '@/components/settings/field-defs'
import { FieldRow } from '@/components/settings/field-row'
import { MemorySection } from '@/components/settings/memory-section'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { SystemToolsPanel } from '@/components/system/system-tools'
import { SystemUpdateCard } from '@/components/system/system-update'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Separator } from '@/components/ui/separator'
import { useToast } from '@/components/ui/sonner'
import {
  type ConfigDiffEntry,
  type ConfigPatchResponse,
  diffConfigs,
  useConfig,
  useSaveConfig,
} from '@/hooks/use-config'
import {
  useEngineConfig,
  useModels,
  useProviders,
  useSetApiKey,
  useSetModel,
  useSetProviderOptions,
} from '@/hooks/use-engine'

export const Route = createFileRoute('/settings')({
  validateSearch: (search: Record<string, unknown>) => ({
    section: (search.section as string) || undefined,
  }),
  component: SettingsPage,
})

// ─── Navigation structure (groups) ────────────────────────────

const navGroups: SubNavGroup[] = [
  {
    id: 'ai',
    labelKey: 'settings.groupAi',
    items: [
      { id: 'engine', labelKey: 'settings.sectionEngine', icon: <Bot className="h-4 w-4" /> },
    ],
  },
  {
    id: 'system',
    labelKey: 'settings.groupSystem',
    items: [
      { id: 'kernel', labelKey: 'settings.sectionKernel', icon: <Cpu className="h-4 w-4" /> },
      { id: 'exec', labelKey: 'settings.sectionExec', icon: <Terminal className="h-4 w-4" /> },
      {
        id: 'scheduler',
        labelKey: 'settings.sectionScheduler',
        icon: <Timer className="h-4 w-4" />,
      },
      {
        id: 'orchestrator',
        labelKey: 'settings.sectionOrchestrator',
        icon: <Zap className="h-4 w-4" />,
      },
      { id: 'context', labelKey: 'settings.sectionContext', icon: <Brain className="h-4 w-4" /> },
      { id: 'gateway', labelKey: 'settings.sectionGateway', icon: <Globe className="h-4 w-4" /> },
      { id: 'session', labelKey: 'settings.sectionSession', icon: <Monitor className="h-4 w-4" /> },
      { id: 'logging', labelKey: 'settings.sectionLogging', icon: <Server className="h-4 w-4" /> },
      { id: 'update', labelKey: 'settings.update', icon: <Sparkles className="h-4 w-4" /> },
    ],
  },
  {
    id: 'security',
    labelKey: 'settings.groupSecurity',
    items: [
      {
        id: 'security',
        labelKey: 'settings.sectionSecurity',
        icon: <Shield className="h-4 w-4" />,
      },
      { id: 'audit', labelKey: 'settings.sectionAudit', icon: <Eye className="h-4 w-4" /> },
    ],
  },
  {
    id: 'memory',
    labelKey: 'settings.groupMemory',
    items: [
      { id: 'memory', labelKey: 'settings.sectionMemory', icon: <Database className="h-4 w-4" /> },
    ],
  },
  {
    id: 'channels',
    labelKey: 'settings.groupChannels',
    items: [
      {
        id: 'channels.telegram',
        labelKey: 'settings.sectionTelegram',
        icon: <Send className="h-4 w-4" />,
      },
    ],
  },
]

// ─── Field definitions (legacy sections from original file) ───

type FieldType = 'text' | 'number' | 'password' | 'toggle' | 'select'

interface LegacyField {
  key: string
  labelKey: string
  descriptionKey: string
  type: FieldType
  placeholder?: string
  options?: { value: string; labelKey: string }[]
  /**
   * Mirrors `SettingsFieldDef.hotReload`. False means the field requires
   * a daemon restart to take effect (the subsystem is constructed at
   * boot). Used by the diff preview badges and the `annotatedDiff`
   * classifier. The two MUST stay in sync with
   * `system.rs::HOT_RELOADABLE_SECTIONS`.
   */
  hotReload: boolean
  /** Sub-system that consumes this value (used in tooltips). */
  restartScope?: 'kernel' | 'gateway' | 'logging' | 'memory' | 'engine' | 'audit'
}

const tKeys = {
  engine: 'settings.engine',
  engineDescription: 'settings.engineDescription',
  provider: 'settings.provider',
  providerDescription: 'settings.providerDescription',
  model: 'settings.model',
  modelDescription: 'settings.modelDescription',
  modelSelectProviderFirst: 'settings.modelSelectProviderFirst',
  apiKey: 'settings.apiKey',
  apiKeyDescription: 'settings.apiKeyDescription',
  advancedOptions: 'settings.advancedOptions',
  advancedOptionsDescription: 'settings.advancedOptionsDescription',
  kernel: 'settings.sectionKernel',
  kernelDescription: 'settings.kernelDescription',
  workspacePath: 'settings.workspacePath',
  workspacePathDescription: 'settings.workspacePathDescription',
  maxConcurrentAgents: 'settings.maxConcurrentAgents',
  maxConcurrentAgentsDescription: 'settings.maxConcurrentAgentsDescription',
  eventBusCapacity: 'settings.eventBusCapacity',
  eventBusCapacityDescription: 'settings.eventBusCapacityDescription',
  execution: 'settings.sectionExec',
  executionDescription: 'settings.executionDescription',
  defaultMode: 'settings.defaultMode',
  defaultModeDescription: 'settings.defaultModeDescription',
  structuredRecommended: 'settings.structuredRecommended',
  shellDangerous: 'settings.shellDangerous',
  allowShellMode: 'settings.allowShellMode',
  allowShellModeDescription: 'settings.allowShellModeDescription',
  defaultTimeoutS: 'settings.defaultTimeoutS',
  defaultTimeoutSDescription: 'settings.defaultTimeoutSDescription',
  maxTimeoutS: 'settings.maxTimeoutS',
  maxTimeoutSDescription: 'settings.maxTimeoutSDescription',
  security: 'settings.sectionSecurity',
  securityDescription: 'settings.securityDescription',
  apiKeyAuthentication: 'settings.apiKeyAuthentication',
  apiKeyAuthenticationDescription: 'settings.apiKeyAuthenticationDescription',
  networkAccess: 'settings.networkAccess',
  networkAccessDescription: 'settings.networkAccessDescription',
  allowForking: 'settings.allowForking',
  allowForkingDescription: 'settings.allowForkingDescription',
  maxExecutionTimeS: 'settings.maxExecutionTimeS',
  maxExecutionTimeSDescription: 'settings.maxExecutionTimeSDescription',
  maxMemoryMB: 'settings.maxMemoryMB',
  maxMemoryMBDescription: 'settings.maxMemoryMBDescription',
  scheduler: 'settings.sectionScheduler',
  schedulerDescription: 'settings.schedulerDescription',
  maxConcurrentTasks: 'settings.maxConcurrentTasks',
  maxConcurrentTasksDescription: 'settings.maxConcurrentTasksDescription',
  rateLimitPerMin: 'settings.rateLimitPerMin',
  rateLimitPerMinDescription: 'settings.rateLimitPerMinDescription',
  zombieTimeoutS: 'settings.zombieTimeoutS',
  zombieTimeoutSDescription: 'settings.zombieTimeoutSDescription',
  orchestrator: 'settings.sectionOrchestrator',
  orchestratorDescription: 'settings.orchestratorDescription',
  maxEvolutionIterations: 'settings.maxEvolutionIterations',
  maxEvolutionIterationsDescription: 'settings.maxEvolutionIterationsDescription',
  minEvaluationScore: 'settings.minEvaluationScore',
  minEvaluationScoreDescription: 'settings.minEvaluationScoreDescription',
  context: 'settings.sectionContext',
  contextDescription: 'settings.contextDescription',
  activeTokenLimit: 'settings.activeTokenLimit',
  activeTokenLimitDescription: 'settings.activeTokenLimitDescription',
  cacheEntryLimit: 'settings.cacheEntryLimit',
  cacheEntryLimitDescription: 'settings.cacheEntryLimitDescription',
  gateway: 'settings.sectionGateway',
  gatewayDescription: 'settings.gatewayDescription',
  host: 'settings.host',
  hostDescription: 'settings.hostDescription',
  port: 'settings.port',
  portDescription: 'settings.portDescription',
  session: 'settings.sectionSession',
  sessionDescription: 'settings.sessionDescription',
  maxSessions: 'settings.maxSessions',
  maxSessionsDescription: 'settings.maxSessionsDescription',
  sessionTTLHours: 'settings.sessionTTLHours',
  sessionTTLHoursDescription: 'settings.sessionTTLHoursDescription',
  autoPrune: 'settings.autoPrune',
  autoPruneDescription: 'settings.autoPruneDescription',
  logging: 'settings.sectionLogging',
  loggingDescription: 'settings.loggingDescription',
  update: 'settings.update',
  updateDescription: 'settings.updateDescription',
  format: 'settings.format',
  formatDescription: 'settings.formatDescription',
  prettyDefault: 'settings.prettyDefault',
  jsonElkLoki: 'settings.jsonElkLoki',
  compact: 'settings.compact',
  title: 'settings.title',
  subtitle: 'settings.subtitle',
} as const

const legacyFieldDefs: [string, string, LegacyField[]][] = [
  [
    'kernel',
    tKeys.kernelDescription,
    [
      // Kernel is constructed at boot; PATCH persists but the running
      // daemon keeps the boot-time values. Restart required.
      {
        key: 'workspace',
        labelKey: tKeys.workspacePath,
        descriptionKey: tKeys.workspacePathDescription,
        type: 'text',
        placeholder: '~/.oxios/workspace',
        hotReload: false,
        restartScope: 'kernel',
      },
      {
        key: 'max_agents',
        labelKey: tKeys.maxConcurrentAgents,
        descriptionKey: tKeys.maxConcurrentAgentsDescription,
        type: 'number',
        placeholder: '10',
        hotReload: false,
        restartScope: 'kernel',
      },
      {
        key: 'event_bus_capacity',
        labelKey: tKeys.eventBusCapacity,
        descriptionKey: tKeys.eventBusCapacityDescription,
        type: 'number',
        placeholder: '256',
        hotReload: false,
        restartScope: 'kernel',
      },
    ],
  ],
  [
    'scheduler',
    tKeys.schedulerDescription,
    [
      // Scheduler is propagated at runtime via `scheduler().update_config()`.
      {
        key: 'max_concurrent',
        labelKey: tKeys.maxConcurrentTasks,
        descriptionKey: tKeys.maxConcurrentTasksDescription,
        type: 'number',
        placeholder: '5',
        hotReload: true,
        restartScope: 'kernel',
      },
      {
        key: 'rate_limit_per_minute',
        labelKey: tKeys.rateLimitPerMin,
        descriptionKey: tKeys.rateLimitPerMinDescription,
        type: 'number',
        placeholder: '60',
        hotReload: true,
        restartScope: 'kernel',
      },
      {
        key: 'zombie_timeout_secs',
        labelKey: tKeys.zombieTimeoutS,
        descriptionKey: tKeys.zombieTimeoutSDescription,
        type: 'number',
        placeholder: '300',
        hotReload: true,
        restartScope: 'kernel',
      },
    ],
  ],
  [
    'orchestrator',
    tKeys.orchestratorDescription,
    [
      // Orchestrator is constructed at boot; PATCH persists but does not
      // re-create it. Restart required.
      {
        key: 'max_evolution_iterations',
        labelKey: tKeys.maxEvolutionIterations,
        descriptionKey: tKeys.maxEvolutionIterationsDescription,
        type: 'number',
        placeholder: '3',
        hotReload: false,
        restartScope: 'kernel',
      },
      {
        key: 'min_evaluation_score',
        labelKey: tKeys.minEvaluationScore,
        descriptionKey: tKeys.minEvaluationScoreDescription,
        type: 'number',
        placeholder: '0.8',
        hotReload: false,
        restartScope: 'kernel',
      },
    ],
  ],
  [
    'context',
    tKeys.contextDescription,
    [
      // Context manager is constructed at boot; restart required.
      {
        key: 'active_limit_tokens',
        labelKey: tKeys.activeTokenLimit,
        descriptionKey: tKeys.activeTokenLimitDescription,
        type: 'number',
        placeholder: '100000',
        hotReload: false,
        restartScope: 'kernel',
      },
      {
        key: 'cache_limit_entries',
        labelKey: tKeys.cacheEntryLimit,
        descriptionKey: tKeys.cacheEntryLimitDescription,
        type: 'number',
        placeholder: '50',
        hotReload: false,
        restartScope: 'kernel',
      },
    ],
  ],
  [
    'gateway',
    tKeys.gatewayDescription,
    [
      // gateway host/port are bound at boot; restart required.
      {
        key: 'host',
        labelKey: tKeys.host,
        descriptionKey: tKeys.hostDescription,
        type: 'text',
        placeholder: '0.0.0.0',
        hotReload: false,
        restartScope: 'gateway',
      },
      {
        key: 'port',
        labelKey: tKeys.port,
        descriptionKey: tKeys.portDescription,
        type: 'number',
        placeholder: '4200',
        hotReload: false,
        restartScope: 'gateway',
      },
    ],
  ],
  [
    'session',
    tKeys.sessionDescription,
    [
      // Session manager is constructed at boot; restart required.
      {
        key: 'max_sessions',
        labelKey: tKeys.maxSessions,
        descriptionKey: tKeys.maxSessionsDescription,
        type: 'number',
        placeholder: '100',
        hotReload: false,
        restartScope: 'kernel',
      },
      {
        key: 'ttl_hours',
        labelKey: tKeys.sessionTTLHours,
        descriptionKey: tKeys.sessionTTLHoursDescription,
        type: 'number',
        placeholder: '168',
        hotReload: false,
        restartScope: 'kernel',
      },
      {
        key: 'auto_prune',
        labelKey: tKeys.autoPrune,
        descriptionKey: tKeys.autoPruneDescription,
        type: 'toggle',
        hotReload: false,
        restartScope: 'kernel',
      },
    ],
  ],
  [
    'logging',
    tKeys.loggingDescription,
    [
      // Logging format is set on the global subscriber at boot; restart required.
      {
        key: 'format',
        labelKey: tKeys.format,
        descriptionKey: tKeys.formatDescription,
        type: 'select',
        options: [
          { value: 'pretty', labelKey: tKeys.prettyDefault },
          { value: 'json', labelKey: tKeys.jsonElkLoki },
          { value: 'compact', labelKey: tKeys.compact },
        ],
        hotReload: false,
        restartScope: 'logging',
      },
    ],
  ],
]

const legacyFieldsBySection = new Map(legacyFieldDefs.map(([key, , fields]) => [key, fields]))
const legacySectionLabelKeys: Record<string, string> = {
  kernel: tKeys.kernel,
  scheduler: tKeys.scheduler,
  orchestrator: tKeys.orchestrator,
  context: tKeys.context,
  gateway: tKeys.gateway,
  session: tKeys.session,
  logging: tKeys.logging,
}

// ─── Engine Panel ────────────────────────────────────────────

function EnginePanel() {
  const { t } = useTranslation()
  const { data: providers = [] } = useProviders()
  const [selectedProvider, setSelectedProvider] = useState<string | null>(null)
  const { data: models = [] } = useModels(selectedProvider)
  const { data: engineConfig } = useEngineConfig()
  const setModel = useSetModel()
  const setApiKey = useSetApiKey()
  const setProviderOptions = useSetProviderOptions()

  const currentModel = engineConfig?.default_model ?? ''
  const resolvedProvider = useMemo((): string | null => {
    if (selectedProvider) return selectedProvider
    if (currentModel.includes('/')) return currentModel.split('/')[0] ?? null
    return null
  }, [selectedProvider, currentModel])

  const currentModelId = useMemo(() => {
    if (!currentModel.includes('/')) return null
    return currentModel  // ModelInfo.id is already "provider/model", used as-is
  }, [currentModel])

  const handleProviderChange = (providerId: string) => {
    setSelectedProvider(providerId)
  }

  const handleModelChange = (modelId: string) => {
    // modelId from ModelSelect is already in "provider/model" format (ModelInfo.id)
    setModel.mutate(modelId)
  }

  const handleApiKeySubmit = (apiKey: string) => {
    setApiKey.mutate({ provider: resolvedProvider ?? 'unknown', apiKey })
  }

  const handleOptionsSave = (options: Record<string, unknown>) => {
    setProviderOptions.mutate({ provider: resolvedProvider ?? 'unknown', options })
  }

  const apiKeySource =
    engineConfig?.api_key_source ?? (engineConfig?.api_key_set ? 'config' : 'none')

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Bot className="h-4 w-4" />
          {t(tKeys.engine)}
        </CardTitle>
        <p className="text-sm text-muted-foreground">{t(tKeys.engineDescription)}</p>
      </CardHeader>
      <CardContent className="space-y-6">
        <div className="flex items-start justify-between gap-6">
          <div className="flex-1 min-w-0 pt-0.5">
            <label className="text-sm font-medium">{t(tKeys.provider)}</label>
            <p className="text-xs text-muted-foreground mt-0.5">{t(tKeys.providerDescription)}</p>
          </div>
          <div className="shrink-0 w-56">
            <ProviderSelect
              providers={providers}
              value={resolvedProvider}
              onValueChange={handleProviderChange}
            />
          </div>
        </div>

        <Separator />

        <div className="flex items-start justify-between gap-6">
          <div className="flex-1 min-w-0 pt-0.5">
            <label className="text-sm font-medium">{t(tKeys.model)}</label>
            <p className="text-xs text-muted-foreground mt-0.5">
              {currentModel ? (
                <span>{t(tKeys.modelDescription, { model: currentModel })}</span>
              ) : (
                t(tKeys.modelSelectProviderFirst)
              )}
            </p>
          </div>
          <div className="shrink-0 w-64">
            <ModelSelect models={models} value={currentModelId} onValueChange={handleModelChange} />
          </div>
        </div>

        <Separator />

        <div className="flex items-start justify-between gap-6">
          <div className="flex-1 min-w-0 pt-0.5">
            <label className="text-sm font-medium">{t(tKeys.apiKey)}</label>
            <p className="text-xs text-muted-foreground mt-0.5">{t(tKeys.apiKeyDescription)}</p>
          </div>
          <div className="shrink-0 w-72">
            <ApiKeyInput
              hasKey={engineConfig?.api_key_set ?? false}
              source={apiKeySource}
              providerName={resolvedProvider ?? 'provider'}
              onSubmit={handleApiKeySubmit}
              isPending={setApiKey.isPending}
            />
          </div>
        </div>

        {resolvedProvider && ['anthropic', 'openai', 'google'].includes(resolvedProvider) && (
          <>
            <Separator />
            <div>
              <div className="mb-3">
                <label className="text-sm font-medium">{t(tKeys.advancedOptions)}</label>
                <p className="text-xs text-muted-foreground mt-0.5">
                  {t(tKeys.advancedOptionsDescription, { provider: resolvedProvider })}
                </p>
              </div>
              <ProviderOptionsPanel
                provider={resolvedProvider}
                onSave={handleOptionsSave}
                isPending={setProviderOptions.isPending}
              />
            </div>
          </>
        )}
      </CardContent>

      <RoutingSection />
    </Card>
  )
}

// ─── Generic Config Section Card (legacy) ─────────────────────

function LegacyConfigSectionCard({
  sectionKey,
  descriptionKey,
  labelKey,
  icon,
  fields,
  formValues,
  onFieldChange,
  config,
}: {
  sectionKey: string
  descriptionKey: string
  labelKey: string
  icon: React.ReactNode
  fields: LegacyField[]
  formValues: Record<string, Record<string, string | boolean>>
  onFieldChange: (sectionKey: string, fieldKey: string, value: string | boolean) => void
  config: Record<string, unknown> | undefined
}) {
  const { t } = useTranslation()
  const sectionConfig = config?.[sectionKey] as Record<string, unknown> | undefined

  if (!sectionConfig && !formValues[sectionKey]) return null

  return (
    <Card>
      <CardHeader className="pb-4">
        <CardTitle className="flex items-center gap-2 text-base">
          {icon}
          {t(labelKey)}
        </CardTitle>
        <p className="text-xs text-muted-foreground">{t(descriptionKey)}</p>
      </CardHeader>
      <CardContent className="space-y-4">
        {fields.map((field, i) => (
          <div key={field.key}>
            {i > 0 && <Separator className="mb-4" />}
            <LegacyFieldRow
              sectionKey={sectionKey}
              field={field}
              value={formValues[sectionKey]?.[field.key]}
              onChange={(val) => onFieldChange(sectionKey, field.key, val)}
            />
          </div>
        ))}
      </CardContent>
    </Card>
  )
}

function LegacyFieldRow({
  sectionKey,
  field,
  value,
  onChange,
}: {
  sectionKey: string
  field: LegacyField
  value: string | boolean | undefined
  onChange: (val: string | boolean) => void
}) {
  const { t } = useTranslation()
  const id = `${sectionKey}-${field.key}`

  return (
    <div className="flex items-start justify-between gap-4 sm:gap-6">
      <div className="flex-1 min-w-0 pt-0.5">
        <label htmlFor={id} className="text-sm font-medium">
          {t(field.labelKey)}
        </label>
        <p className="text-xs text-muted-foreground mt-0.5">{t(field.descriptionKey)}</p>
      </div>
      <div className="shrink-0 w-40 sm:w-56">
        {field.type === 'toggle' ? (
          <div className="flex items-center justify-end gap-2">
            <span className="text-xs text-muted-foreground">
              {value ? t('common.on') : t('common.off')}
            </span>
            <Switch checked={Boolean(value)} onCheckedChange={(checked) => onChange(checked)} />
          </div>
        ) : field.type === 'select' ? (
          <Select
            value={String(value ?? '')}
            onValueChange={(v) => onChange(v)}
            placeholder={t(field.labelKey)}
            options={
              field.options?.map((opt) => ({
                label: t(opt.labelKey),
                value: opt.value,
              })) ?? []
            }
            className="w-full"
          />
        ) : (
          <Input
            id={id}
            type={
              field.type === 'password' ? 'password' : field.type === 'number' ? 'number' : 'text'
            }
            value={String(value ?? '')}
            onChange={(e) => onChange(e.target.value)}
            placeholder={field.placeholder}
          />
        )}
      </div>
    </div>
  )
}

// ─── Settings page ───────────────────────────────────────────

// Map a section's fields to the storage key used in formValues
// (`sectionKey.fieldKey` for simple sections, or `sectionKey.subKey` for
// nested sections like memory.* and channels.telegram.*).

function getNestedValue(obj: Record<string, unknown>, dotted: string): unknown {
  const parts = dotted.split('.')
  let cur: unknown = obj
  for (const p of parts) {
    if (cur && typeof cur === 'object') {
      cur = (cur as Record<string, unknown>)[p]
    } else {
      return undefined
    }
  }
  return cur
}

function setNestedValue(obj: Record<string, unknown>, dotted: string, value: unknown): void {
  const parts = dotted.split('.')
  let cur: Record<string, unknown> = obj
  for (let i = 0; i < parts.length - 1; i++) {
    const p = parts[i]!
    if (cur[p] === null || typeof cur[p] !== 'object') {
      cur[p] = {}
    }
    cur = cur[p] as Record<string, unknown>
  }
  cur[parts[parts.length - 1]!] = value
}

function SettingsPage() {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const search = useSearch({ from: '/settings' })
  const [activeSection, setActiveSection] = useState(search?.section ?? 'engine')
  const [showDiff, setShowDiff] = useState(false)
  // `formValues` is keyed by sectionKey (e.g. `memory`, `channels.telegram`)
  // and contains arbitrary nested objects for those sections.
  const [formValues, setFormValues] = useState<Record<string, Record<string, unknown>>>({})
  const [saveNotice, setSaveNotice] = useState<'success' | 'error' | null>(null)

  const { data: config, isLoading, isError, refetch } = useConfig()
  const saveMutation = useSaveConfig()

  // Populate form from server config (initial sync + when navigating sections).
  useEffect(() => {
    if (!config) return
    const next: Record<string, Record<string, unknown>> = {}

    // Legacy sections (simple flat keys).
    for (const [sectionKey, , fields] of legacyFieldDefs) {
      const sectionConfig = config[sectionKey] as Record<string, unknown> | undefined
      if (!sectionConfig) continue
      const bucket: Record<string, unknown> = {}
      for (const field of fields) {
        const raw = sectionConfig[field.key]
        bucket[field.key] =
          field.type === 'toggle' ? raw === true || raw === 'true' : String(raw ?? '')
      }
      next[sectionKey] = bucket
    }

    // New sections with potentially nested keys.
    for (const section of NEW_SECTIONS) {
      const bucket: Record<string, unknown> = {}
      for (const field of section.fields) {
        // `memory.*` keys (and other nested sections) are dotted.
        // Resolve them against the right sub-object. `channels.telegram`
        // uses a flat key under the `telegram` sub-object — the section
        // key is prepended at payload-build time.
        const dottedKey = field.key
        let container: Record<string, unknown> | undefined
        if (section.key === 'memory') {
          const [sub, ...rest] = dottedKey.split('.')
          container = config.memory as Record<string, unknown> | undefined
          if (container && sub && rest.length > 0) {
            container = container[sub] as Record<string, unknown> | undefined
          } else if (container && sub) {
            // Top-level memory field (e.g. `enabled`).
            const topVal = container?.[sub]
            bucket[dottedKey] = field.type === 'toggle' ? topVal === true : String(topVal ?? '')
            continue
          }
          if (container) {
            const leaf = rest.join('.')
            const v = leaf ? getNestedValue(container, leaf) : undefined
            bucket[dottedKey] = field.type === 'toggle' ? v === true : String(v ?? '')
          }
        } else if (section.key === 'channels.telegram') {
          // The field key is the path *below* `channels.telegram`. The
          // section key is prepended at payload-build time, so we read
          // from `config.channels.telegram.<key>` directly.
          const tg = (config.channels as Record<string, unknown> | undefined)?.telegram as
            | Record<string, unknown>
            | undefined
          if (tg) {
            const v = getNestedValue(tg, dottedKey)
            bucket[dottedKey] = field.type === 'toggle' ? v === true : String(v ?? '')
          }
        } else {
          // Flat sections (exec, security, audit).
          const sectionConfig = config[section.key] as Record<string, unknown> | undefined
          const raw = sectionConfig?.[dottedKey]
          bucket[dottedKey] = field.type === 'toggle' ? raw === true : String(raw ?? '')
        }
      }
      next[section.key] = bucket
    }

    setFormValues(next)
  }, [config])

  // Build the PATCH payload from current form state.
  const buildPayload = (): Record<string, unknown> => {
    const payload: Record<string, unknown> = {}

    // Legacy sections.
    for (const [sectionKey, , fields] of legacyFieldDefs) {
      const sectionValues: Record<string, unknown> = {}
      for (const field of fields) {
        const val = formValues[sectionKey]?.[field.key]
        if (val === undefined) continue
        if (field.type === 'toggle') sectionValues[field.key] = Boolean(val)
        else if (field.type === 'number') {
          const num = Number(val)
          if (!Number.isNaN(num) && val !== '') sectionValues[field.key] = num
        } else if (val !== '') sectionValues[field.key] = val
      }
      payload[sectionKey] = sectionValues
    }

    // New sections. Each section's `field.key` is the path *below* the
    // section key, so we collect values into `bucket` and assign once
    // to `payload[section.key]`. This matches how `memory` and
    // `channels.telegram` are encoded in `OxiosConfig`.
    for (const section of NEW_SECTIONS) {
      const bucket: Record<string, unknown> = {}
      for (const field of section.fields) {
        const raw = formValues[section.key]?.[field.key]
        if (raw === undefined || raw === '') continue
        if (field.type === 'toggle') {
          setNestedValue(bucket, field.key, Boolean(raw))
        } else if (field.type === 'tags') {
          // String array (commands).
          const arr = Array.isArray(raw)
            ? raw
            : String(raw)
                .split(/[\s,]+/)
                .filter(Boolean)
          setNestedValue(bucket, field.key, arr)
        } else if (field.type === 'csv') {
          const arr = String(raw)
            .split(',')
            .map((s) => s.trim())
            .filter(Boolean)
          setNestedValue(bucket, field.key, arr)
        } else if (field.type === 'numbers') {
          const arr = String(raw)
            .split('\n')
            .map((s) => Number(s.trim()))
            .filter((n) => !Number.isNaN(n))
          setNestedValue(bucket, field.key, arr)
        } else if (field.type === 'number') {
          const num = Number(raw)
          if (!Number.isNaN(num)) setNestedValue(bucket, field.key, num)
        } else {
          setNestedValue(bucket, field.key, String(raw))
        }
      }
      // Assign bucket under the section key. We use `setNestedValue` so
      // section keys that contain dots (e.g. `channels.telegram`)
      // become a nested object literal, not a flat key with a dot in
      // its name. `obj['channels.telegram'] = x` would otherwise set
      // `obj["channels.telegram"]` and the server would receive a
      // payload whose top-level key is `"channels.telegram"` instead
      // of `obj.channels.telegram`.
      setNestedValue(payload, section.key, bucket)
    }

    return payload
  }

  // Compute the current diff against the last-loaded config.
  const diff: ConfigDiffEntry[] = useMemo(() => {
    if (!config) return []
    const proposed = buildPayload()
    return diffConfigs(config, proposed)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [config, formValues, activeSection])

  // Re-classify the diff with the authoritative hot-reload metadata from
  // field-defs (this is local & instant; the backend re-checks authoritatively).
  const annotatedDiff: ConfigDiffEntry[] = useMemo(() => {
    return diff.map((entry) => {
      // Try to find the matching field def in the new sections.
      for (const section of NEW_SECTIONS) {
        const f = section.fields.find(
          (field) => field.key === entry.path || `${section.key}.${field.key}` === entry.path,
        )
        if (f) {
          return { ...entry, hotReload: f.hotReload, scope: f.restartScope }
        }
      }
      // Also try legacy fields. They declare their own hotReload and
      // restartScope so the diff badges and tooltips match the backend.
      for (const [sectionKey, , fields] of legacyFieldDefs) {
        const f = (fields as LegacyField[]).find(
          (field) => `${sectionKey}.${field.key}` === entry.path,
        )
        if (f) {
          return { ...entry, hotReload: f.hotReload, scope: f.restartScope }
        }
      }
      // Unknown path — let the backend classification in the response
      // handle it. Mark hotReload: false to err on the safe side.
      return { ...entry, hotReload: false }
    })
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [diff])

  const hasUnsaved = annotatedDiff.length > 0

  const { toast } = useToast()

  const handleSaveClick = () => {
    if (annotatedDiff.length === 0) return
    setShowDiff(true)
  }

  const handleConfirmSave = () => {
    const payload = buildPayload()
    saveMutation.mutate(payload, {
      onSuccess: (res) => {
        setShowDiff(false)
        setSaveNotice('success')
        setTimeout(() => setSaveNotice(null), 3000)
        if (res && 'hot_reload' in res) {
          const r = (res as ConfigPatchResponse).hot_reload
          // Surface the hot-reload breakdown as a toast so the user
          // gets a visible confirmation of what was applied and what
          // needs a restart.
          if (r.requires_restart.length > 0) {
            toast(
              t('settings.savedWithRestart', {
                applied: r.applied_immediately.length,
                restart: r.requires_restart.length,
              }),
              'default',
            )
          } else if (r.applied_immediately.length > 0) {
            toast(t('settings.savedApplied', { count: r.applied_immediately.length }), 'success')
          } else {
            toast(t('settings.settingsSaved'), 'success')
          }
        } else {
          toast(t('settings.settingsSaved'), 'success')
        }
        queryClient.invalidateQueries({ queryKey: ['config'] })
      },
      onError: () => {
        setShowDiff(false)
        setSaveNotice('error')
        setTimeout(() => setSaveNotice(null), 5000)
        toast(t('settings.settingsSaveFailed'), 'destructive')
      },
    })
  }

  const setField = (sectionKey: string, fieldKey: string, value: unknown) => {
    setFormValues((prev) => ({
      ...prev,
      [sectionKey]: { ...(prev[sectionKey] ?? {}), [fieldKey]: value },
    }))
  }

  if (isLoading) return <LoadingCards count={4} />
  if (isError) return <ErrorState onRetry={() => refetch()} />

  // Render active panel content.
  const renderContent = () => {
    if (activeSection === 'engine') return <EnginePanel />
    if (activeSection === 'update') {
      return (
        <div className="space-y-6">
          <SystemUpdateCard />
          <SystemToolsPanel />
        </div>
      )
    }

    // New sections.
    const newSection = NEW_SECTIONS.find((s) => s.key === activeSection)
    if (newSection) {
      return renderNewSection(newSection, formValues, setField, t)
    }

    // Legacy sections.
    const fields = legacyFieldsBySection.get(activeSection)
    if (!fields) return null
    const def = legacyFieldDefs.find(([key]) => key === activeSection)
    if (!def) return null
    return (
      <LegacyConfigSectionCard
        sectionKey={activeSection}
        labelKey={legacySectionLabelKeys[activeSection] ?? activeSection}
        descriptionKey={def[1]}
        icon={navGroups.flatMap((g) => g.items).find((i) => i.id === activeSection)?.icon}
        fields={fields}
        formValues={formValues as Record<string, Record<string, string | boolean>>}
        onFieldChange={(sk, fk, v) => setField(sk, fk, v)}
        config={config}
      />
    )
  }

  return (
    <div>
      {/* Header */}
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-bold">{t(tKeys.title)}</h1>
          <p className="text-muted-foreground text-sm">{t(tKeys.subtitle)}</p>
        </div>
      </div>

      {/* Save notice */}
      {saveNotice === 'success' && (
        <div className="rounded-lg border border-success-subtle p-3 text-sm text-success mb-4">
          {t('settings.settingsSaved')}
        </div>
      )}
      {saveNotice === 'error' && (
        <div className="rounded-lg border border-error-subtle p-3 text-sm text-error mb-4">
          {t('settings.settingsSaveFailed')}
        </div>
      )}

      <SettingsLayout groups={navGroups} activeId={activeSection} onNavigate={setActiveSection}>
        {renderContent()}
      </SettingsLayout>

      {/* Sticky Save Bar */}
      <div
        className="sticky bottom-0 -mx-4 sm:-mx-6 mt-8 border-t bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/80 px-4 sm:px-6 py-3 z-10"
        data-testid="sticky-save-bar"
      >
        <div className="flex items-center justify-between gap-3 max-w-3xl">
          <div className="text-xs text-muted-foreground">
            {hasUnsaved ? (
              <span>
                {t('settings.unsavedChanges')} · {annotatedDiff.length}{' '}
                {annotatedDiff.length === 1 ? 'field' : 'fields'}
              </span>
            ) : (
              <span>{t('settings.settingsSaved')}</span>
            )}
          </div>
          <div className="flex items-center gap-2">
            <Button
              variant="outline"
              size="sm"
              disabled={!hasUnsaved || saveMutation.isPending}
              onClick={() => refetch()}
              data-testid="reset-changes"
            >
              {t('common.reset')}
            </Button>
            <Button
              size="sm"
              disabled={!hasUnsaved || saveMutation.isPending}
              onClick={handleSaveClick}
              data-testid="save-changes"
            >
              <Save className="h-4 w-4 mr-2" />
              {saveMutation.isPending ? t('settings.savingChanges') : t('common.save')}
            </Button>
          </div>
        </div>
      </div>

      {/* Diff Preview Modal */}
      <DiffPreview
        open={showDiff}
        onOpenChange={setShowDiff}
        diffs={annotatedDiff}
        onConfirm={handleConfirmSave}
        isPending={saveMutation.isPending}
      />
    </div>
  )
}

// ─── New section renderer ────────────────────────────────────

function renderNewSection(
  section: SettingsSectionDef,
  formValues: Record<string, Record<string, unknown>>,
  setField: (sectionKey: string, fieldKey: string, value: unknown) => void,
  t: (key: string) => string,
) {
  // Memory gets sub-cards.
  if (section.key === 'memory') {
    const fieldsBySubsection: Record<string, SettingsFieldDef[]> = {
      storage: section.fields.filter((f) => f.key === 'enabled' || f.key.startsWith('sqlite.')),
      embedding: section.fields.filter((f) => f.key.startsWith('embedding.')),
      learning: section.fields.filter((f) => f.key.startsWith('learning.')),
      dream: section.fields.filter((f) => f.key.startsWith('consolidation.')),
    }
    return (
      <MemorySection
        fieldsBySubsection={fieldsBySubsection}
        formValues={formValues as Record<string, Record<string, string | boolean>>}
        onFieldChange={(sk, fk, v) => setField(sk, fk, v)}
      />
    )
  }

  // Channels get a single card.
  if (section.key === 'channels.telegram') {
    return (
      <ChannelsSection
        sectionKey={section.key}
        labelKey={section.labelKey}
        fields={section.fields}
        formValues={formValues as Record<string, Record<string, string | boolean | string[]>>}
        onFieldChange={(sk, fk, v) => setField(sk, fk, v)}
      />
    )
  }

  // Default: a single card with all fields.
  return (
    <Card>
      <CardHeader className="pb-4">
        <CardTitle className="flex items-center gap-2 text-base">
          <MessageSquare className="h-4 w-4" />
          {t(section.labelKey)}
        </CardTitle>
        <p className="text-xs text-muted-foreground">{t(section.descriptionKey)}</p>
      </CardHeader>
      <CardContent className="space-y-4">
        {section.fields.map((field, i) => (
          <div key={field.key}>
            {i > 0 && <Separator className="mb-4" />}
            <FieldRow
              sectionKey={section.key}
              field={field}
              value={
                formValues[section.key]?.[field.key] as
                  | string
                  | boolean
                  | string[]
                  | number
                  | undefined
              }
              onChange={(val) => setField(section.key, field.key, val)}
            />
          </div>
        ))}
      </CardContent>
    </Card>
  )
}

import { Input } from '@/components/ui/input'
import { Select } from '@/components/ui/select'
// Imports that need to be available in this file.
import { Switch } from '@/components/ui/switch'
