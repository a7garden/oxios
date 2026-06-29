import { useQueryClient } from '@tanstack/react-query'
import { createFileRoute, useSearch } from '@tanstack/react-router'
import { useEffect, useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { ModelSelect } from '@/components/engine/model-select'
import { AddProviderCard, ProviderCard } from '@/components/engine/provider-card'
import { ProviderOptionsPanel } from '@/components/engine/provider-options'
import { RoleSection } from '@/components/engine/role-section'
import { RoutingSection } from '@/components/engine/routing-section'
import { ChannelsSection } from '@/components/settings/channels-section'
import { DiffPreview } from '@/components/settings/diff-preview'
import {
  getSectionMeta,
  NEW_SECTIONS,
  pathLabelMap,
  SECTION_META,
  SETTINGS_GROUPS,
  type SectionMeta,
  type SettingsFieldDef,
} from '@/components/settings/field-defs'
import { FieldRow } from '@/components/settings/field-row'
import { MemorySection } from '@/components/settings/memory-section'
import { NotificationSectionCard } from '@/components/settings/notification-section'
import { SaveDock } from '@/components/settings/save-dock'
import { SecretsSectionCard } from '@/components/settings/secrets-section'
import { SectionCard } from '@/components/settings/section-card'
import { SectionIcon } from '@/components/settings/section-icons'
import { SettingsHeader } from '@/components/settings/settings-header'
import { SettingsShell } from '@/components/settings/settings-shell'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { SystemToolsPanel } from '@/components/system/system-tools'
import { SystemUpdateCard } from '@/components/system/system-update'
import { Separator } from '@/components/ui/separator'
import {
  type ConfigDiffEntry,
  type ConfigPatchResponse,
  diffConfigs,
  useConfig,
  useSaveConfig,
} from '@/hooks/use-config'
import {
  useDeleteApiKey,
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

// ─── Field type re-declaration (matches old LegacyField exactly) ──
//
// The legacy sections are rendered using the new `FieldRow` /
// `SectionCard` components, but their declarations still live in this
// file because they share form-state plumbing with the new sections.

type FieldType = 'text' | 'number' | 'password' | 'toggle' | 'select' | 'range'

interface LegacyField {
  key: string
  labelKey: string
  descriptionKey: string
  type: FieldType
  placeholder?: string
  options?: { value: string; labelKey: string }[]
  hotReload: boolean
  restartScope?: 'kernel' | 'gateway' | 'logging' | 'memory' | 'engine' | 'audit'
  min?: number
  max?: number
  step?: number
}

const tKeys = {
  kernel: 'settings.kernel',
  kernelDescription: 'settings.kernelDescription',
  workspacePath: 'settings.workspacePath',
  workspacePathDescription: 'settings.workspacePathDescription',
  maxConcurrentAgents: 'settings.maxConcurrentAgents',
  maxConcurrentAgentsDescription: 'settings.maxConcurrentAgentsDescription',
  eventBusCapacity: 'settings.eventBusCapacity',
  eventBusCapacityDescription: 'settings.eventBusCapacityDescription',
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
  title: 'settings.title',
  subtitle: 'settings.subtitle',
} as const

const legacyFieldDefs: [string, LegacyField[]][] = [
  [
    'kernel',
    [
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
        type: 'range',
        min: 1,
        max: 50,
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
    'orchestrator',
    [
      {
        key: 'max_evolution_iterations',
        labelKey: tKeys.maxEvolutionIterations,
        descriptionKey: tKeys.maxEvolutionIterationsDescription,
        type: 'range',
        min: 1,
        max: 10,
        placeholder: '3',
        hotReload: false,
        restartScope: 'kernel',
      },
      {
        key: 'min_evaluation_score',
        labelKey: tKeys.minEvaluationScore,
        descriptionKey: tKeys.minEvaluationScoreDescription,
        type: 'range',
        min: 0,
        max: 1,
        step: 0.05,
        placeholder: '0.8',
        hotReload: false,
        restartScope: 'kernel',
      },
    ],
  ],
  [
    'context',
    [
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
        type: 'range',
        min: 5,
        max: 200,
        step: 5,
        placeholder: '50',
        hotReload: false,
        restartScope: 'kernel',
      },
    ],
  ],
  [
    'gateway',
    [
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
    [
      {
        key: 'max_sessions',
        labelKey: tKeys.maxSessions,
        descriptionKey: tKeys.maxSessionsDescription,
        type: 'range',
        min: 10,
        max: 500,
        step: 10,
        placeholder: '100',
        hotReload: false,
        restartScope: 'kernel',
      },
      {
        key: 'ttl_hours',
        labelKey: tKeys.sessionTTLHours,
        descriptionKey: tKeys.sessionTTLHoursDescription,
        type: 'range',
        min: 1,
        max: 720,
        step: 24,
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
    [
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

const legacyFieldsBySection = new Map(legacyFieldDefs.map(([key, fields]) => [key, fields]))

// Merged lookup: dotted config path → i18n label key, for both legacy
// and new sections. Passed to DiffPreview so the review dialog shows
// human-readable labels instead of raw config paths.
const diffLabelMap: Map<string, string> = (() => {
  const m = new Map(pathLabelMap)
  for (const [sectionKey, fields] of legacyFieldDefs) {
    for (const field of fields) {
      m.set(`${sectionKey}.${field.key}`, field.labelKey)
    }
  }
  return m
})()

// ─── Form-state primitives ──────────────────────────────────────

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

// ─── Engine Panel (custom-rendered section) ─────────────────────

function EnginePanel() {
  const { t } = useTranslation()
  const { data: providers = [] } = useProviders()
  const { data: engineConfig } = useEngineConfig()
  const setModel = useSetModel()
  const setApiKey = useSetApiKey()
  const deleteApiKey = useDeleteApiKey()
  const setProviderOptions = useSetProviderOptions()

  const currentModel = engineConfig?.default_model ?? ''
  const currentModelProvider = currentModel.includes('/')
    ? (currentModel.split('/')[0] ?? null)
    : null

  const { data: models = [] } = useModels(null)

  const connected = providers.filter((p) => p.hasKey)
  const available = providers.filter((p) => !p.hasKey)
  const providersById = useMemo(
    () => new Map(providers.map((p) => [p.id, p.name] as const)),
    [providers],
  )
  const isMutating = setApiKey.isPending || deleteApiKey.isPending || setModel.isPending

  const handleAdd = (provider: string, apiKey: string) => {
    setApiKey.mutate(
      { provider, apiKey },
      {
        onSuccess: () => toast.success(t('engine.connected')),
        onError: () => toast.error(t('common.error')),
      },
    )
  }

  const handleChangeKey = (provider: string, apiKey: string) => {
    setApiKey.mutate({ provider, apiKey })
  }

  const handleRemove = (provider: string) => {
    deleteApiKey.mutate(provider, {
      onSuccess: () => toast.success(t('common.success')),
    })
  }

  const handleModelChange = (modelId: string) => {
    setModel.mutate(modelId)
  }

  const handleOptionsSave = (options: Record<string, unknown>) => {
    setProviderOptions.mutate({ provider: currentModelProvider ?? 'unknown', options })
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <Bot className="h-4 w-4" />
          {t('settings.engine')}
        </CardTitle>
        <p className="text-sm text-muted-foreground">{t('settings.engineDescription')}</p>
      </CardHeader>
      <CardContent className="space-y-6">
        {/* ── Connected Providers ── */}
        <div>
          <div className="mb-3">
            <label className="text-sm font-medium">{t('engine.connectedProviders')}</label>
            <p className="text-xs text-muted-foreground mt-0.5">
              {t('engine.connectedProvidersDesc')}
            </p>
          </div>
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3 auto-rows-fr">
            {connected.map((p) => (
              <ProviderCard
                key={p.id}
                provider={p}
                onChangeKey={(key) => handleChangeKey(p.id, key)}
                onRemove={() => handleRemove(p.id)}
                isPending={isMutating}
              />
            ))}
            {connected.length === 0 && (
              <div className="flex flex-col items-center justify-center rounded-lg border border-dashed p-6 text-center min-h-[124px]">
                <p className="text-sm text-muted-foreground">{t('engine.noProvidersConnected')}</p>
                <p className="text-xs text-muted-foreground mt-1">
                  {t('engine.noProvidersConnectedDesc')}
                </p>
              </div>
            )}
            <AddProviderCard
              availableProviders={available}
              onAdd={handleAdd}
              isPending={setApiKey.isPending}
            />
          </div>
        </div>

        <Separator />

        {/* ── Default Model ── */}
        {connected.length > 0 && (
          <div className="flex items-start justify-between gap-6">
            <div className="flex-1 min-w-0 pt-0.5">
              <label className="text-sm font-medium">{t('engine.defaultModel')}</label>
              <p className="text-xs text-muted-foreground mt-0.5">
                {currentModel
                  ? t('settings.modelDescription', { model: currentModel })
                  : t('engine.defaultModelDesc')}
              </p>
            </div>
            <div className="shrink-0 w-64">
              <ModelSelect
                models={models}
                value={currentModel}
                onValueChange={handleModelChange}
                providersById={providersById}
              />
            </div>
          </div>
        )}

        {/* ── Advanced Options ── */}
        {currentModelProvider &&
          ['anthropic', 'openai', 'google'].includes(currentModelProvider) && (
            <>
              <Separator />
              <div>
                <div className="mb-3">
                  <label className="text-sm font-medium">{t('settings.advancedOptions')}</label>
                  <p className="text-xs text-muted-foreground mt-0.5">
                    {t('settings.advancedOptionsDescription', { provider: currentModelProvider })}
                  </p>
                </div>
                <ProviderOptionsPanel
                  provider={currentModelProvider}
                  onSave={handleOptionsSave}
                  isPending={setProviderOptions.isPending}
                />
              </div>
            </>
          )}
        <RoutingSection />
        <RoleSection />
      </CardContent>
    </Card>
  )
}

// ─── Card / header / input imports scoped to the EnginePanel ───
//
// We import them lazily at the bottom to keep the engine panel close to
// the other legacy form code that uses the same primitives.
import { Bot } from 'lucide-react'
import { AllowedToolsPicker } from '@/components/settings/allowed-tools-picker'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { validateCorsOrigin } from '@/lib/cors-validator'

// ─── Settings Page ─────────────────────────────────────────────

function SettingsPage() {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const search = useSearch({ from: '/settings' })
  const [activeSection, setActiveSection] = useState(search?.section ?? 'engine')
  const [showDiff, setShowDiff] = useState(false)
  const [formValues, setFormValues] = useState<Record<string, Record<string, unknown>>>({})
  const [lastSavedAt, setLastSavedAt] = useState<Date | null>(null)

  const { data: config, isLoading, isError, refetch } = useConfig()
  const saveMutation = useSaveConfig()

  // Deep-link recovery: if the URL points at a section that isn't in
  // SECTION_META (e.g. an unimplemented id like `persona`), fall back
  // to the first section so we never render a blank screen.
  const safeActiveSection = getSectionMeta(activeSection)
    ? activeSection
    : (SECTION_META[0]?.id ?? 'engine')

  // Populate form from server config (initial sync + when navigating sections).
  useEffect(() => {
    if (!config) return
    const next: Record<string, Record<string, unknown>> = {}

    // Legacy sections (simple flat keys).
    for (const [sectionKey, fields] of legacyFieldDefs) {
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
        const dottedKey = field.key
        if (section.key === 'memory') {
          const [sub, ...rest] = dottedKey.split('.')
          let container: Record<string, unknown> | undefined = config.memory as
            | Record<string, unknown>
            | undefined
          if (container && sub && rest.length > 0) {
            container = container[sub] as Record<string, unknown> | undefined
          } else if (container && sub) {
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
          const tg = (config.channels as Record<string, unknown> | undefined)?.telegram as
            | Record<string, unknown>
            | undefined
          if (tg) {
            const v = getNestedValue(tg, dottedKey)
            bucket[dottedKey] = field.type === 'toggle' ? v === true : String(v ?? '')
          }
        } else {
          const sectionConfig = config[section.key] as Record<string, unknown> | undefined
          const raw = sectionConfig?.[dottedKey]
          bucket[dottedKey] =
            field.type === 'toggle'
              ? raw === true
              : field.type === 'multiline' && typeof raw === 'object'
                ? JSON.stringify(raw, null, 2)
                : String(raw ?? '')
        }
      }
      next[section.key] = bucket
    }

    setFormValues(next)
  }, [config])

  const buildPayload = (): Record<string, unknown> => {
    const payload: Record<string, unknown> = {}

    for (const [sectionKey, fields] of legacyFieldDefs) {
      const sectionValues: Record<string, unknown> = {}
      for (const field of fields) {
        const val = formValues[sectionKey]?.[field.key]
        if (val === undefined) continue
        if (field.type === 'toggle') sectionValues[field.key] = Boolean(val)
        else if (field.type === 'number' || field.type === 'range') {
          const num = Number(val)
          if (!Number.isNaN(num) && val !== '') sectionValues[field.key] = num
        } else if (val !== '') sectionValues[field.key] = val
      }
      payload[sectionKey] = sectionValues
    }

    for (const section of NEW_SECTIONS) {
      const bucket: Record<string, unknown> = {}
      for (const field of section.fields) {
        const raw = formValues[section.key]?.[field.key]
        if (raw === undefined || raw === '') continue
        if (field.type === 'toggle') {
          setNestedValue(bucket, field.key, Boolean(raw))
        } else if (field.type === 'tags') {
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
        } else if (field.type === 'number' || field.type === 'range') {
          const num = Number(raw)
          if (!Number.isNaN(num)) setNestedValue(bucket, field.key, num)
        } else if (field.type === 'multiline') {
          // Try to parse as JSON (for fields like browser.engine);
          // fall back to the raw string if it's not valid JSON.
          try {
            setNestedValue(bucket, field.key, JSON.parse(String(raw)))
          } catch {
            setNestedValue(bucket, field.key, String(raw))
          }
        }
      }
      setNestedValue(payload, section.key, bucket)
    }

    return payload
  }

  const diff: ConfigDiffEntry[] = useMemo(() => {
    if (!config) return []
    const proposed = buildPayload()
    return diffConfigs(config, proposed)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [config, formValues])

  const annotatedDiff: ConfigDiffEntry[] = useMemo(() => {
    return diff.map((entry) => {
      for (const section of NEW_SECTIONS) {
        const f = section.fields.find(
          (field) => field.key === entry.path || `${section.key}.${field.key}` === entry.path,
        )
        if (f) {
          return { ...entry, hotReload: f.hotReload, scope: f.restartScope }
        }
      }
      for (const [sectionKey, fields] of legacyFieldDefs) {
        const f = fields.find((field) => `${sectionKey}.${field.key}` === entry.path)
        if (f) {
          return { ...entry, hotReload: f.hotReload, scope: f.restartScope }
        }
      }
      return { ...entry, hotReload: false }
    })
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [diff])

  const hasUnsaved = annotatedDiff.length > 0
  const restartCount = annotatedDiff.filter((d) => !d.hotReload).length
  const hotReloadCount = annotatedDiff.length - restartCount

  // Map unsaved diffs to per-section counts. Used by the rail to show
  // badges and modified dots, and by the section card headers.
  const unsavedBySection = useMemo(() => {
    const out: Record<string, number> = {}
    for (const entry of annotatedDiff) {
      const top = entry.path.split('.')[0] ?? entry.path
      out[top] = (out[top] ?? 0) + 1
    }
    return out
  }, [annotatedDiff])

  const handleSaveClick = () => {
    if (annotatedDiff.length === 0) return
    setShowDiff(true)
  }

  const handleConfirmSave = () => {
    const payload = buildPayload()
    saveMutation.mutate(payload, {
      onSuccess: (res) => {
        setShowDiff(false)
        setLastSavedAt(new Date())
        if (res && 'hot_reload' in res) {
          const r = (res as ConfigPatchResponse).hot_reload
          if (r.requires_restart.length > 0) {
            toast(
              t('settings.savedWithRestart', {
                applied: r.applied_immediately.length,
                restart: r.requires_restart.length,
              }),
            )
          } else if (r.applied_immediately.length > 0) {
            toast.success(t('settings.savedApplied', { count: r.applied_immediately.length }))
          } else {
            toast.success(t('settings.settingsSaved'))
          }
        } else {
          toast.success(t('settings.settingsSaved'))
        }
        queryClient.invalidateQueries({ queryKey: ['config'] })
      },
      onError: () => {
        setShowDiff(false)
        toast.error(t('settings.settingsSaveFailed'))
      },
    })
  }

  const handleDiscard = () => {
    setFormValues({})
    refetch()
  }

  const setField = (sectionKey: string, fieldKey: string, value: unknown) => {
    setFormValues((prev) => ({
      ...prev,
      [sectionKey]: { ...(prev[sectionKey] ?? {}), [fieldKey]: value },
    }))
  }

  if (isLoading) return <LoadingCards count={4} />
  if (isError) return <ErrorState onRetry={() => refetch()} />

  const handleNavigate = (id: string) => {
    setActiveSection(id)
    // Keep URL in sync so deep-links work.
    const url = new URL(window.location.href)
    url.searchParams.set('section', id)
    window.history.replaceState({}, '', url.toString())
  }

  // Build shell data from SECTION_META + SETTINGS_GROUPS.
  const shellGroups = SETTINGS_GROUPS.filter((g) =>
    SECTION_META.some((s) => s.groupId === g.id),
  ).map((g) => ({ id: g.id, labelKey: g.labelKey }))

  const shellSections = SECTION_META.map((m) => ({
    id: m.id,
    labelKey: m.labelKey,
    groupId: m.groupId,
  }))

  const activeMeta = getSectionMeta(safeActiveSection)

  return (
    <div>
      {/* Header */}
      <SettingsHeader
        title={t(tKeys.title)}
        subtitle={t(tKeys.subtitle)}
        status={
          saveMutation.isPending
            ? 'saving'
            : saveMutation.isError
              ? 'error'
              : hasUnsaved
                ? 'unsaved'
                : 'saved'
        }
        lastSavedAt={lastSavedAt}
        unsavedCount={annotatedDiff.length}
      />

      <SettingsShell
        groups={shellGroups}
        sections={shellSections}
        activeId={safeActiveSection}
        onNavigate={handleNavigate}
        unsavedBySection={unsavedBySection}
        onReview={hasUnsaved ? handleSaveClick : undefined}
      >
        {renderActiveSection(
          safeActiveSection,
          activeMeta,
          formValues,
          setField,
          t,
          hasUnsaved,
          unsavedBySection[safeActiveSection] ?? 0,
          handleDiscard,
        )}
      </SettingsShell>

      {/* Floating Save Dock — the single source of save/discard actions.
          Rendered `position: fixed` so it stays reachable regardless of
          scroll position. A previous inline fallback button block was
          removed to avoid a duplicate CTA (review feedback). */}
      <SaveDock
        totalChanges={annotatedDiff.length}
        restartRequired={restartCount}
        applyLive={hotReloadCount}
        isPending={saveMutation.isPending}
        onReview={handleSaveClick}
        onDiscard={handleDiscard}
        visible={hasUnsaved}
      />

      {/* Diff Preview Modal */}
      <DiffPreview
        open={showDiff}
        onOpenChange={setShowDiff}
        diffs={annotatedDiff}
        onConfirm={handleConfirmSave}
        isPending={saveMutation.isPending}
        labelForPath={(path) => diffLabelMap.get(path)}
      />
    </div>
  )
}

// ─── Section renderer (unified for legacy + new) ────────────────

function renderActiveSection(
  sectionId: string,
  meta: SectionMeta | undefined,
  formValues: Record<string, Record<string, unknown>>,
  setField: (sectionKey: string, fieldKey: string, value: unknown) => void,
  t: (key: string) => string,
  _hasUnsaved: boolean,
  unsavedCount: number,
  onDiscardAll: () => void,
) {
  // Engine: render the dedicated engine panel.
  if (sectionId === 'engine') {
    return <EnginePanel />
  }

  // Update: dedicated system update card.
  if (sectionId === 'update') {
    return (
      <div className="space-y-4">
        <SystemUpdateCard />
        <SystemToolsPanel />
      </div>
    )
  }

  // Secrets: dedicated secrets management card (RFC-028 SP-2c).
  if (sectionId === 'secrets') {
    return <SecretsSectionCard />
  }

  // Notifications: client-side prefs card (RFC-028 SP-1e).
  if (sectionId === 'notifications') {
    return <NotificationSectionCard />
  }

  if (!meta) return null

  // Memory: render sub-cards for storage / embedding / learning / dream.
  if (sectionId === 'memory') {
    const memorySection = NEW_SECTIONS.find((s) => s.key === 'memory')!
    const fieldsBySubsection: Record<string, SettingsFieldDef[]> = {
      storage: memorySection.fields.filter(
        (f) => f.key === 'enabled' || f.key.startsWith('sqlite.'),
      ),
      embedding: memorySection.fields.filter((f) => f.key.startsWith('embedding.')),
      learning: memorySection.fields.filter((f) => f.key.startsWith('learning.')),
      dream: memorySection.fields.filter((f) => f.key.startsWith('consolidation.')),
    }
    return (
      <MemorySection
        fieldsBySubsection={fieldsBySubsection}
        formValues={formValues as Record<string, Record<string, string | boolean | string[]>>}
        onFieldChange={(sk, fk, v) => setField(sk, fk, v)}
      />
    )
  }

  // Telegram (channel): dedicated channels section card.
  if (sectionId === 'channels.telegram') {
    const tg = NEW_SECTIONS.find((s) => s.key === 'channels.telegram')!
    return (
      <ChannelsSection
        sectionKey={tg.key}
        labelKey={tg.labelKey}
        fields={tg.fields}
        formValues={formValues as Record<string, Record<string, string | boolean | string[]>>}
        onFieldChange={(sk, fk, v) => setField(sk, fk, v)}
      />
    )
  }

  // Security: dedicated SectionCard with AllowedToolsPicker + CORS validation.
  if (sectionId === 'security') {
    return (
      <SecuritySectionCard
        securityValues={formValues.security}
        onFieldChange={(fk, v) => setField('security', fk, v)}
        onDiscardAll={onDiscardAll}
        unsavedCount={unsavedCount}
      />
    )
  }

  // New sections: render a unified SectionCard.
  const newSection = NEW_SECTIONS.find((s) => s.key === sectionId)
  if (newSection) {
    return (
      <SectionCard
        title={t(newSection.labelKey)}
        description={t(newSection.descriptionKey)}
        icon={<SectionIcon iconKey={newSection.iconKey} className="h-3.5 w-3.5" />}
        sectionId={newSection.key}
        fieldCount={newSection.fields.length}
        modified={unsavedCount > 0}
        onReset={onDiscardAll}
      >
        {newSection.fields.map((field) => (
          <FieldRow
            key={field.key}
            sectionKey={newSection.key}
            field={field}
            value={
              formValues[newSection.key]?.[field.key] as
                | string
                | boolean
                | string[]
                | number
                | undefined
            }
            onChange={(val) => setField(newSection.key, field.key, val)}
            modified={unsavedCount > 0 && formValues[newSection.key]?.[field.key] !== undefined}
            sectionValues={formValues[newSection.key]}
          />
        ))}
      </SectionCard>
    )
  }

  // Legacy sections: render a unified SectionCard with the legacy
  // field defs mapped into the SettingsFieldDef shape.
  const legacyFields = legacyFieldsBySection.get(sectionId)
  if (legacyFields) {
    return (
      <LegacySectionCard
        meta={meta}
        sectionId={sectionId}
        fields={legacyFields}
        formValues={formValues}
        setField={setField}
        t={t}
        modified={unsavedCount > 0}
        onReset={onDiscardAll}
      />
    )
  }

  return null
}

function LegacySectionCard({
  meta,
  sectionId,
  fields,
  formValues,
  setField,
  t,
  modified,
  onReset,
}: {
  meta: SectionMeta
  sectionId: string
  fields: LegacyField[]
  formValues: Record<string, Record<string, unknown>>
  setField: (sectionKey: string, fieldKey: string, value: unknown) => void
  t: (key: string) => string
  modified: boolean
  onReset: () => void
}) {
  return (
    <SectionCard
      title={t(meta.labelKey)}
      description={t(meta.descriptionKey)}
      icon={<SectionIcon iconKey={meta.iconKey} className="h-3.5 w-3.5" />}
      sectionId={sectionId}
      fieldCount={fields.length}
      modified={modified}
      onReset={onReset}
    >
      {fields.map((field) => {
        // Adapt LegacyField → SettingsFieldDef so we can reuse the new
        // FieldRow component (which is the source of truth for the
        // responsive layout + RestartBadge).
        const adapted: SettingsFieldDef = {
          key: field.key,
          labelKey: field.labelKey,
          descriptionKey: field.descriptionKey,
          type: field.type,
          placeholder: field.placeholder,
          options: field.options,
          hotReload: field.hotReload,
          restartScope: field.restartScope,
          min: field.min,
          max: field.max,
          step: field.step,
        }
        const v = formValues[sectionId]?.[field.key]
        return (
          <FieldRow
            key={field.key}
            sectionKey={sectionId}
            field={adapted}
            value={v as string | boolean | string[] | number | undefined}
            onChange={(val) => setField(sectionId, field.key, val)}
            sectionValues={formValues[sectionId]}
          />
        )
      })}
    </SectionCard>
  )
}

// ─── SecuritySectionCard ─────────────────────────────────────

/** Security section card with AllowedToolsPicker + CORS validation. */
function SecuritySectionCard({
  securityValues,
  onFieldChange,
  onDiscardAll,
  unsavedCount,
}: {
  securityValues?: Record<string, unknown>
  onFieldChange: (fieldKey: string, value: unknown) => void
  onDiscardAll: () => void
  unsavedCount: number
}) {
  const { t } = useTranslation()
  const section = NEW_SECTIONS.find((s) => s.key === 'security')!

  return (
    <SectionCard
      title={t('settings.sectionSecurity')}
      description={t('settings.securityDescription')}
      icon={<SectionIcon iconKey="security" className="h-3.5 w-3.5" />}
      sectionId="security"
      fieldCount={section.fields.length}
      modified={unsavedCount > 0}
      onReset={onDiscardAll}
    >
      {section.fields.map((field) => {
        const v = securityValues?.[field.key]
        const isAllowedTools = field.key === 'allowed_tools'
        const isCorsOrigins = field.key === 'cors_origins'

        if (isAllowedTools) {
          return (
            <div key={field.key} className="space-y-2">
              <div className="min-w-0">
                <div className="flex items-center gap-2 flex-wrap">
                  <label className="text-sm font-medium text-foreground">{t(field.labelKey)}</label>
                </div>
                <p className="mt-1 text-xs text-muted-foreground leading-relaxed">
                  {t(field.descriptionKey)}
                </p>
              </div>
              <AllowedToolsPicker
                value={Array.isArray(v) ? (v as string[]) : []}
                onChange={(next) => onFieldChange(field.key, next)}
              />
            </div>
          )
        }

        return (
          <div key={field.key}>
            <FieldRow
              sectionKey="security"
              field={field}
              value={
                securityValues?.[field.key] as string | boolean | string[] | number | undefined
              }
              onChange={(val) => onFieldChange(field.key, val)}
              sectionValues={securityValues}
              validate={isCorsOrigins ? validateCorsOrigin : undefined}
            />
          </div>
        )
      })}
    </SectionCard>
  )
}

// ─── End ──────────────────────────────────────────────────────
