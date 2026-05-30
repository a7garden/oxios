import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import {
  Bot,
  Brain,
  Cpu,
  Globe,
  Monitor,
  Save,
  Server,
  Shield,
  Terminal,
  Timer,
  Zap,
} from 'lucide-react'
import { useEffect, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { ProviderSelect } from '@/components/engine/provider-select'
import { ModelSelect } from '@/components/engine/model-select'
import { ApiKeyInput } from '@/components/engine/api-key-input'
import { ProviderOptionsPanel } from '@/components/engine/provider-options'
import { RoutingSection } from '@/components/engine/routing-section'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Separator } from '@/components/ui/separator'
import { Switch } from '@/components/ui/switch'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import {
  useProviders,
  useModels,
  useEngineConfig,
  useSetModel,
  useSetApiKey,
  useSetProviderOptions,
} from '@/hooks/use-engine'
import { api } from '@/lib/api-client'

export const Route = createFileRoute('/settings')({ component: SettingsPage })

// ─── Translation keys ─────────────────────────────────────────
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
  kernel: 'settings.kernel',
  kernelDescription: 'settings.kernelDescription',
  workspacePath: 'settings.workspacePath',
  workspacePathDescription: 'settings.workspacePathDescription',
  maxConcurrentAgents: 'settings.maxConcurrentAgents',
  maxConcurrentAgentsDescription: 'settings.maxConcurrentAgentsDescription',
  eventBusCapacity: 'settings.eventBusCapacity',
  eventBusCapacityDescription: 'settings.eventBusCapacityDescription',
  execution: 'settings.execution',
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
  security: 'settings.security',
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
  scheduler: 'settings.scheduler',
  schedulerDescription: 'settings.schedulerDescription',
  maxConcurrentTasks: 'settings.maxConcurrentTasks',
  maxConcurrentTasksDescription: 'settings.maxConcurrentTasksDescription',
  rateLimitPerMin: 'settings.rateLimitPerMin',
  rateLimitPerMinDescription: 'settings.rateLimitPerMinDescription',
  zombieTimeoutS: 'settings.zombieTimeoutS',
  zombieTimeoutSDescription: 'settings.zombieTimeoutSDescription',
  orchestrator: 'settings.orchestrator',
  orchestratorDescription: 'settings.orchestratorDescription',
  maxEvolutionIterations: 'settings.maxEvolutionIterations',
  maxEvolutionIterationsDescription: 'settings.maxEvolutionIterationsDescription',
  minEvaluationScore: 'settings.minEvaluationScore',
  minEvaluationScoreDescription: 'settings.minEvaluationScoreDescription',
  context: 'settings.context',
  contextDescription: 'settings.contextDescription',
  activeTokenLimit: 'settings.activeTokenLimit',
  activeTokenLimitDescription: 'settings.activeTokenLimitDescription',
  cacheEntryLimit: 'settings.cacheEntryLimit',
  cacheEntryLimitDescription: 'settings.cacheEntryLimitDescription',
  gateway: 'settings.gateway',
  gatewayDescription: 'settings.gatewayDescription',
  host: 'settings.host',
  hostDescription: 'settings.hostDescription',
  port: 'settings.port',
  portDescription: 'settings.portDescription',
  session: 'settings.session',
  sessionDescription: 'settings.sessionDescription',
  maxSessions: 'settings.maxSessions',
  maxSessionsDescription: 'settings.maxSessionsDescription',
  sessionTTLHours: 'settings.sessionTTLHours',
  sessionTTLHoursDescription: 'settings.sessionTTLHoursDescription',
  autoPrune: 'settings.autoPrune',
  autoPruneDescription: 'settings.autoPruneDescription',
  logging: 'settings.logging',
  loggingDescription: 'settings.loggingDescription',
  format: 'settings.format',
  formatDescription: 'settings.formatDescription',
  prettyDefault: 'settings.prettyDefault',
  jsonElkLoki: 'settings.jsonElkLoki',
  compact: 'settings.compact',
} as const

// ─── Engine Panel ────────────────────────────────────────────

/**
 * Dedicated Engine settings panel with rich provider/model selection.
 * Replaces the old raw-text fields with interactive components.
 */
function EnginePanel() {
  const { t } = useTranslation()
  const { data: providers = [] } = useProviders()
  const [selectedProvider, setSelectedProvider] = useState<string | null>(null)
  const { data: models = [] } = useModels(selectedProvider)
  const { data: engineConfig } = useEngineConfig()
  const setModel = useSetModel()
  const setApiKey = useSetApiKey()
  const setProviderOptions = useSetProviderOptions()

  // Derive provider from current default_model on load
  const currentModel = engineConfig?.default_model ?? ''
  const resolvedProvider = useMemo((): string | null => {
    if (selectedProvider) return selectedProvider
    if (currentModel.includes('/')) return currentModel.split('/')[0] ?? null
    return null
  }, [selectedProvider, currentModel])

  // Current model ID (without provider prefix)
  const currentModelId = useMemo(() => {
    if (!currentModel.includes('/')) return null
    return currentModel.split('/').slice(1).join('/')
  }, [currentModel])

  const handleProviderChange = (providerId: string) => {
    setSelectedProvider(providerId)
  }

  const handleModelChange = (modelId: string) => {
    const provider = resolvedProvider ?? 'unknown'
    const fullId = `${provider}/${modelId}`
    setModel.mutate(fullId)
  }

  const handleApiKeySubmit = (apiKey: string) => {
    setApiKey.mutate({ provider: resolvedProvider ?? 'unknown', apiKey })
  }

  const handleOptionsSave = (options: Record<string, unknown>) => {
    setProviderOptions.mutate({ provider: resolvedProvider ?? 'unknown', options })
  }

  // Determine API key source for display
  const apiKeySource = engineConfig?.api_key_set ? 'config' : 'none'

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Bot className="h-4 w-4" />
          {t(tKeys.engine)}
        </CardTitle>
        <p className="text-sm text-muted-foreground">
          {t(tKeys.engineDescription)}
        </p>
      </CardHeader>
      <CardContent className="space-y-6">
        {/* Provider selection */}
        <div className="flex items-start justify-between gap-6">
          <div className="flex-1 min-w-0 pt-0.5">
            <label className="text-sm font-medium">{t(tKeys.provider)}</label>
            <p className="text-xs text-muted-foreground mt-0.5">
              {t(tKeys.providerDescription)}
            </p>
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

        {/* Model selection */}
        <div className="flex items-start justify-between gap-6">
          <div className="flex-1 min-w-0 pt-0.5">
            <label className="text-sm font-medium">{t(tKeys.model)}</label>
            <p className="text-xs text-muted-foreground mt-0.5">
              {currentModel ? (
                <span>
                  {t(tKeys.modelDescription, { model: currentModel })}
                </span>
              ) : (
                t(tKeys.modelSelectProviderFirst)
              )}
            </p>
          </div>
          <div className="shrink-0 w-64">
            <ModelSelect
              models={models}
              value={currentModelId}
              onValueChange={handleModelChange}
            />
          </div>
        </div>

        <Separator />

        {/* API Key */}
        <div className="flex items-start justify-between gap-6">
          <div className="flex-1 min-w-0 pt-0.5">
            <label className="text-sm font-medium">{t(tKeys.apiKey)}</label>
            <p className="text-xs text-muted-foreground mt-0.5">
              {t(tKeys.apiKeyDescription)}
            </p>
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

        {/* Provider-specific options */}
        {resolvedProvider &&
          ['anthropic', 'openai', 'google'].includes(resolvedProvider) && (
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

      {/* Routing section — model routing configuration (RFC-011) */}
      <RoutingSection />
    </Card>
  )
}

// ─── Field types ─────────────────────────────────────────────

type FieldType = 'text' | 'number' | 'password' | 'toggle' | 'select'

interface SettingsField {
  key: string
  labelKey: string
  descriptionKey: string
  type: FieldType
  placeholder?: string
  /** For select type */
  options?: { value: string; labelKey: string }[]
}

interface SettingsSection {
  key: string
  labelKey: string
  descriptionKey: string
  icon: React.ReactNode
  fields: SettingsField[]
}

// ─── Section definitions (matches config.toml schema) ────────

const sections: SettingsSection[] = [
  {
    key: 'kernel',
    labelKey: tKeys.kernel,
    descriptionKey: tKeys.kernelDescription,
    icon: <Cpu className="h-4 w-4" />,
    fields: [
      {
        key: 'workspace',
        labelKey: tKeys.workspacePath,
        descriptionKey: tKeys.workspacePathDescription,
        type: 'text',
        placeholder: '~/.oxios/workspace',
      },
      {
        key: 'max_agents',
        labelKey: tKeys.maxConcurrentAgents,
        descriptionKey: tKeys.maxConcurrentAgentsDescription,
        type: 'number',
        placeholder: '10',
      },
      {
        key: 'event_bus_capacity',
        labelKey: tKeys.eventBusCapacity,
        descriptionKey: tKeys.eventBusCapacityDescription,
        type: 'number',
        placeholder: '256',
      },
    ],
  },
  {
    key: 'exec',
    labelKey: tKeys.execution,
    descriptionKey: tKeys.executionDescription,
    icon: <Terminal className="h-4 w-4" />,
    fields: [
      {
        key: 'default_mode',
        labelKey: tKeys.defaultMode,
        descriptionKey: tKeys.defaultModeDescription,
        type: 'select',
        options: [
          { value: 'structured', labelKey: tKeys.structuredRecommended },
          { value: 'shell', labelKey: tKeys.shellDangerous },
        ],
      },
      {
        key: 'allow_shell_mode',
        labelKey: tKeys.allowShellMode,
        descriptionKey: tKeys.allowShellModeDescription,
        type: 'toggle',
      },
      {
        key: 'default_timeout_secs',
        labelKey: tKeys.defaultTimeoutS,
        descriptionKey: tKeys.defaultTimeoutSDescription,
        type: 'number',
        placeholder: '120',
      },
      {
        key: 'max_timeout_secs',
        labelKey: tKeys.maxTimeoutS,
        descriptionKey: tKeys.maxTimeoutSDescription,
        type: 'number',
        placeholder: '600',
      },
    ],
  },
  {
    key: 'security',
    labelKey: tKeys.security,
    descriptionKey: tKeys.securityDescription,
    icon: <Shield className="h-4 w-4" />,
    fields: [
      {
        key: 'auth_enabled',
        labelKey: tKeys.apiKeyAuthentication,
        descriptionKey: tKeys.apiKeyAuthenticationDescription,
        type: 'toggle',
      },
      {
        key: 'network_access',
        labelKey: tKeys.networkAccess,
        descriptionKey: tKeys.networkAccessDescription,
        type: 'toggle',
      },
      {
        key: 'can_fork',
        labelKey: tKeys.allowForking,
        descriptionKey: tKeys.allowForkingDescription,
        type: 'toggle',
      },
      {
        key: 'max_execution_time_secs',
        labelKey: tKeys.maxExecutionTimeS,
        descriptionKey: tKeys.maxExecutionTimeSDescription,
        type: 'number',
        placeholder: '300',
      },
      {
        key: 'max_memory_mb',
        labelKey: tKeys.maxMemoryMB,
        descriptionKey: tKeys.maxMemoryMBDescription,
        type: 'number',
        placeholder: '512',
      },
    ],
  },
  {
    key: 'scheduler',
    labelKey: tKeys.scheduler,
    descriptionKey: tKeys.schedulerDescription,
    icon: <Timer className="h-4 w-4" />,
    fields: [
      {
        key: 'max_concurrent',
        labelKey: tKeys.maxConcurrentTasks,
        descriptionKey: tKeys.maxConcurrentTasksDescription,
        type: 'number',
        placeholder: '5',
      },
      {
        key: 'rate_limit_per_minute',
        labelKey: tKeys.rateLimitPerMin,
        descriptionKey: tKeys.rateLimitPerMinDescription,
        type: 'number',
        placeholder: '60',
      },
      {
        key: 'zombie_timeout_secs',
        labelKey: tKeys.zombieTimeoutS,
        descriptionKey: tKeys.zombieTimeoutSDescription,
        type: 'number',
        placeholder: '300',
      },
    ],
  },
  {
    key: 'orchestrator',
    labelKey: tKeys.orchestrator,
    descriptionKey: tKeys.orchestratorDescription,
    icon: <Zap className="h-4 w-4" />,
    fields: [
      {
        key: 'max_evolution_iterations',
        labelKey: tKeys.maxEvolutionIterations,
        descriptionKey: tKeys.maxEvolutionIterationsDescription,
        type: 'number',
        placeholder: '3',
      },
      {
        key: 'min_evaluation_score',
        labelKey: tKeys.minEvaluationScore,
        descriptionKey: tKeys.minEvaluationScoreDescription,
        type: 'number',
        placeholder: '0.8',
      },
    ],
  },
  {
    key: 'context',
    labelKey: tKeys.context,
    descriptionKey: tKeys.contextDescription,
    icon: <Brain className="h-4 w-4" />,
    fields: [
      {
        key: 'active_limit_tokens',
        labelKey: tKeys.activeTokenLimit,
        descriptionKey: tKeys.activeTokenLimitDescription,
        type: 'number',
        placeholder: '100000',
      },
      {
        key: 'cache_limit_entries',
        labelKey: tKeys.cacheEntryLimit,
        descriptionKey: tKeys.cacheEntryLimitDescription,
        type: 'number',
        placeholder: '50',
      },
    ],
  },
  {
    key: 'gateway',
    labelKey: tKeys.gateway,
    descriptionKey: tKeys.gatewayDescription,
    icon: <Globe className="h-4 w-4" />,
    fields: [
      {
        key: 'host',
        labelKey: tKeys.host,
        descriptionKey: tKeys.hostDescription,
        type: 'text',
        placeholder: '0.0.0.0',
      },
      {
        key: 'port',
        labelKey: tKeys.port,
        descriptionKey: tKeys.portDescription,
        type: 'number',
        placeholder: '4200',
      },
    ],
  },
  {
    key: 'session',
    labelKey: tKeys.session,
    descriptionKey: tKeys.sessionDescription,
    icon: <Monitor className="h-4 w-4" />,
    fields: [
      {
        key: 'max_sessions',
        labelKey: tKeys.maxSessions,
        descriptionKey: tKeys.maxSessionsDescription,
        type: 'number',
        placeholder: '100',
      },
      {
        key: 'ttl_hours',
        labelKey: tKeys.sessionTTLHours,
        descriptionKey: tKeys.sessionTTLHoursDescription,
        type: 'number',
        placeholder: '168',
      },
      {
        key: 'auto_prune',
        labelKey: tKeys.autoPrune,
        descriptionKey: tKeys.autoPruneDescription,
        type: 'toggle',
      },
    ],
  },
  {
    key: 'logging',
    labelKey: tKeys.logging,
    descriptionKey: tKeys.loggingDescription,
    icon: <Server className="h-4 w-4" />,
    fields: [
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
      },
    ],
  },
]

// ─── Settings page ───────────────────────────────────────────

function SettingsPage() {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const [formValues, setFormValues] = useState<Record<string, Record<string, string | boolean>>>({})
  const [activeTab, setActiveTab] = useState('kernel')
  const [saveNotice, setSaveNotice] = useState<'success' | 'error' | null>(null)

  const {
    data: config,
    isLoading,
    isError,
    refetch,
  } = useQuery({
    queryKey: ['config'],
    queryFn: () => api.get<Record<string, unknown>>('/api/config'),
  })

  const saveMutation = useMutation({
    mutationFn: (updated: Record<string, unknown>) => api.put('/api/config', updated),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['config'] })
      setSaveNotice('success')
      setTimeout(() => setSaveNotice(null), 3000)
    },
    onError: () => {
      setSaveNotice('error')
      setTimeout(() => setSaveNotice(null), 5000)
    },
  })

  // Populate form from API response
  useEffect(() => {
    if (!config) return
    const values: Record<string, Record<string, string | boolean>> = {}
    for (const section of sections) {
      const sectionConfig = config[section.key] as Record<string, unknown> | undefined
      if (!sectionConfig) continue
      values[section.key] = {}
      for (const field of section.fields) {
        const raw = sectionConfig[field.key]
        if (field.type === 'toggle') {
          values[section.key]![field.key] = raw === true || raw === 'true'
        } else {
          values[section.key]![field.key] = String(raw ?? '')
        }
      }
    }
    setFormValues(values)
  }, [config])

  const handleSave = () => {
    const updated: Record<string, unknown> = {}
    for (const section of sections) {
      const sectionValues: Record<string, unknown> = {}
      for (const field of section.fields) {
        const val = formValues[section.key]?.[field.key]
        if (val === undefined) continue
        if (field.type === 'toggle') {
          sectionValues[field.key] = Boolean(val)
        } else if (field.type === 'number') {
          const num = Number(val)
          if (!isNaN(num) && val !== '') sectionValues[field.key] = num
        } else {
          if (val !== '') sectionValues[field.key] = val
        }
      }
      updated[section.key] = sectionValues
    }
    saveMutation.mutate(updated)
  }

  const setField = (sectionKey: string, fieldKey: string, value: string | boolean) => {
    setFormValues((prev) => ({
      ...prev,
      [sectionKey]: { ...prev[sectionKey], [fieldKey]: value },
    }))
  }

  if (isLoading) return <LoadingCards count={4} />
  if (isError) return <ErrorState onRetry={() => refetch()} />

  return (
    <div className="space-y-6 max-w-4xl">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">{t(tKeys.title)}</h1>
          <p className="text-muted-foreground">
            {t(tKeys.subtitle)}
          </p>
        </div>
        <Button onClick={handleSave} disabled={saveMutation.isPending}>
          <Save className="h-4 w-4 mr-2" />
          {saveMutation.isPending ? t('settings.saving') : t('common.save')}
        </Button>
      </div>

      {/* Save notice */}
      {saveNotice === 'success' && (
        <div className="rounded-lg border border-emerald-200 bg-emerald-50 dark:border-emerald-900 dark:bg-emerald-950 p-3 text-sm text-emerald-700 dark:text-emerald-400">
          {t('settings.settingsSavedSuccessfully')}
        </div>
      )}
      {saveNotice === 'error' && (
        <div className="rounded-lg border border-red-200 bg-red-50 dark:border-red-900 dark:bg-red-950 p-3 text-sm text-red-700 dark:text-red-400">
          {t('settings.failedToSaveSettings')}
        </div>
      )}

      {/* Tabs */}
      <Tabs>
        <TabsList className="flex-wrap h-auto gap-1">
          {/* Engine tab (always first) */}
          <TabsTrigger
            data-state={activeTab === 'engine' ? 'active' : 'inactive'}
            onClick={() => setActiveTab('engine')}
            className="gap-1.5"
          >
            <Bot className="h-4 w-4" />
            <span>{t(tKeys.engine)}</span>
          </TabsTrigger>
          {sections.map((s) => (
            <TabsTrigger
              key={s.key}
              data-state={activeTab === s.key ? 'active' : 'inactive'}
              onClick={() => setActiveTab(s.key)}
              className="gap-1.5"
            >
              {s.icon}
              <span>{t(s.labelKey)}</span>
            </TabsTrigger>
          ))}
        </TabsList>

        {/* Engine tab content */}
        <TabsContent value="engine">
          <EnginePanel />
        </TabsContent>

        {/* Generic section tabs */}
        {sections.map((section) => {
          const sectionConfig = config?.[section.key] as Record<string, unknown> | undefined
          if (!sectionConfig && !formValues[section.key]) return null

          return (
            <TabsContent key={section.key} value={section.key}>
              <Card>
                <CardHeader>
                  <CardTitle className="flex items-center gap-2">
                    {section.icon}
                    {t(section.labelKey)}
                  </CardTitle>
                  <p className="text-sm text-muted-foreground">{t(section.descriptionKey)}</p>
                </CardHeader>
                <CardContent className="space-y-6">
                  {section.fields.map((field, i) => (
                    <div key={field.key}>
                      {i > 0 && <Separator className="mb-6" />}
                      <FieldRow
                        sectionKey={section.key}
                        field={field}
                        value={formValues[section.key]?.[field.key]}
                        onChange={(val) => setField(section.key, field.key, val)}
                      />
                    </div>
                  ))}
                </CardContent>
              </Card>
            </TabsContent>
          )
        })}
      </Tabs>
    </div>
  )
}

// ─── Field row ───────────────────────────────────────────────

function FieldRow({
  sectionKey,
  field,
  value,
  onChange,
}: {
  sectionKey: string
  field: SettingsField
  value: string | boolean | undefined
  onChange: (val: string | boolean) => void
}) {
  const { t } = useTranslation()
  const id = `${sectionKey}-${field.key}`

  return (
    <div className="flex items-start justify-between gap-6">
      {/* Label + description */}
      <div className="flex-1 min-w-0 pt-0.5">
        <label htmlFor={id} className="text-sm font-medium">
          {t(field.labelKey)}
        </label>
        <p className="text-xs text-muted-foreground mt-0.5">{t(field.descriptionKey)}</p>
      </div>

      {/* Control */}
      <div className="shrink-0 w-56">
        {field.type === 'toggle' ? (
          <div className="flex items-center justify-end gap-2">
            <span className="text-xs text-muted-foreground">
              {value ? t('common.on') : t('common.off')}
            </span>
            <Switch
              id={id}
              checked={Boolean(value)}
              onCheckedChange={(checked) => onChange(checked)}
            />
          </div>
        ) : field.type === 'select' ? (
          <select
            id={id}
            value={String(value ?? '')}
            onChange={(e) => onChange(e.target.value)}
            className="flex h-9 w-full rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
          >
            {field.options?.map((opt) => (
              <option key={opt.value} value={opt.value}>
                {t(opt.labelKey)}
              </option>
            ))}
          </select>
        ) : (
          <Input
            id={id}
            type={field.type === 'password' ? 'password' : field.type === 'number' ? 'number' : 'text'}
            value={String(value ?? '')}
            onChange={(e) => onChange(e.target.value)}
            placeholder={field.placeholder}
          />
        )}
      </div>
    </div>
  )
}