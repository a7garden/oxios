// Field definitions for the Settings UI.
//
// Each entry is a triple:
//   [sectionKey, descriptionKey, fields[]]
//
// `field.hotReload` mirrors the backend classification in
// `src/api/routes/system.rs::is_restart_required`. The two
// MUST stay in sync — the field-defs version is the source of truth for
// the UI (badges, diff preview) and the backend is the source of truth
// for the actual hot-reload behaviour.
//
// Sections not listed here (engine, kernel legacy fields, logging legacy
// fields) fall back to the original `routes/settings.tsx` definitions.

export type FieldType =
  | 'text'
  | 'number'
  | 'password'
  | 'toggle'
  | 'select'
  | 'multiline'
  | 'csv' // comma-separated list (e.g. cors_origins)
  | 'tags' // multi-line tag list (e.g. allowed_commands)
  | 'numbers' // multi-line number list (e.g. telegram allowed_users)
  | 'range' // bounded numeric slider (requires min/max)

export interface SettingsFieldDependsOn {
  /**
   * Dotted key of the parent field within the same section.
   * Example: for memory, `'consolidation.dream_enabled'`.
   */
  field: string
  /**
   * The value the parent must have for this field to be enabled.
   * For toggle parents: `true` (or `false` for inverse logic).
   * For select parents: the activating string value, e.g. `'enforced'`.
   */
  value: boolean | string
}

export interface SettingsFieldDef {
  /** Dotted config key, e.g. `exec.allowed_commands` or `memory.embedding.provider`. */
  key: string
  /** i18n key for the field label. */
  labelKey: string
  /** i18n key for the field description. */
  descriptionKey: string
  /** Form control type. */
  type: FieldType
  /** Placeholder text. */
  placeholder?: string
  /** For `select` fields. */
  options?: { value: string; labelKey: string }[]
  /** Minimum value (for `number` and `range` types). */
  min?: number
  /** Maximum value (for `range` types; required when type is `range`). */
  max?: number
  /** Step increment (for `range` types; defaults to 1). */
  step?: number
  /** If false, the field requires a daemon restart to take effect. */
  hotReload: boolean
  /** Sub-system that consumes this value (used in tooltips). */
  restartScope?: 'kernel' | 'gateway' | 'logging' | 'memory' | 'engine' | 'audit'
  /**
   * If set, this field is disabled when the parent field
   * does NOT match `value`. The parent is looked up from
   * the same section's form values (passed as `sectionValues`
   * to `<FieldRow />`).
   */
  dependsOn?: SettingsFieldDependsOn
}

export interface SettingsSectionDef {
  key: string
  labelKey: string
  descriptionKey: string
  iconKey: SectionIconKey
  /** Group id this section belongs to. */
  groupId: 'ai' | 'system' | 'security' | 'memory' | 'channels'
  fields: SettingsFieldDef[]
}

// ---------------------------------------------------------------------------
// 1. exec — Execution
// ---------------------------------------------------------------------------

const execSection: SettingsSectionDef = {
  key: 'exec',
  labelKey: 'settings.execution',
  descriptionKey: 'settings.executionDescription',
  iconKey: 'exec',
  groupId: 'security',
  fields: [
    {
      key: 'default_mode',
      labelKey: 'settings.defaultMode',
      descriptionKey: 'settings.defaultModeDescription',
      type: 'select',
      options: [
        { value: 'structured', labelKey: 'settings.structuredRecommended' },
        { value: 'shell', labelKey: 'settings.shellDangerous' },
      ],
      hotReload: true,
    },
    {
      key: 'allow_shell_mode',
      labelKey: 'settings.allowShellMode',
      descriptionKey: 'settings.allowShellModeDescription',
      type: 'toggle',
      hotReload: true,
    },
    {
      key: 'allowed_commands',
      labelKey: 'settings.allowedCommands',
      descriptionKey: 'settings.allowedCommandsDescription',
      type: 'tags',
      hotReload: true,
      restartScope: 'kernel',
      dependsOn: { field: 'allowlist_mode', value: 'enforced' },
    },
    {
      key: 'allowlist_mode',
      labelKey: 'settings.allowlistMode',
      descriptionKey: 'settings.allowlistModeDescription',
      type: 'select',
      options: [
        { value: 'permissive', labelKey: 'settings.allowlistModePermissive' },
        { value: 'enforced', labelKey: 'settings.allowlistModeEnforced' },
      ],
      hotReload: true,
      restartScope: 'kernel',
    },
    {
      key: 'default_timeout_secs',
      labelKey: 'settings.defaultTimeoutS',
      descriptionKey: 'settings.defaultTimeoutSDescription',
      type: 'range',
      min: 10,
      max: 600,
      step: 10,
      placeholder: '120',
      hotReload: true,
    },
    {
      key: 'max_timeout_secs',
      labelKey: 'settings.maxTimeoutS',
      descriptionKey: 'settings.maxTimeoutSDescription',
      type: 'range',
      min: 30,
      max: 3600,
      step: 30,
      placeholder: '600',
      hotReload: true,
    },
  ],
}

// ---------------------------------------------------------------------------
// 2. security — Security / RBAC
// ---------------------------------------------------------------------------

const securitySection: SettingsSectionDef = {
  key: 'security',
  labelKey: 'settings.security',
  descriptionKey: 'settings.securityDescription',
  iconKey: 'security',
  groupId: 'security',
  fields: [
    {
      key: 'auth_enabled',
      labelKey: 'settings.apiKeyAuthentication',
      descriptionKey: 'settings.apiKeyAuthenticationDescription',
      type: 'toggle',
      // The security subsystem is constructed at boot. PATCH on this
      // section persists the new value but the running AccessManager
      // keeps using the boot-time value. Restart is required to apply.
      hotReload: false,
      restartScope: 'gateway',
    },
    {
      key: 'allowed_tools',
      labelKey: 'settings.allowedTools',
      descriptionKey: 'settings.allowedToolsDescription',
      type: 'tags',
      placeholder: 'read, write, edit, bash',
      hotReload: false,
      restartScope: 'gateway',
    },
    {
      key: 'cors_origins',
      labelKey: 'settings.corsOrigins',
      descriptionKey: 'settings.corsOriginsDescription',
      type: 'tags',
      placeholder: 'http://localhost:4200, http://localhost:3000',
      hotReload: false,
      restartScope: 'gateway',
    },
    {
      key: 'network_access',
      labelKey: 'settings.networkAccess',
      descriptionKey: 'settings.networkAccessDescription',
      type: 'toggle',
      hotReload: false,
      restartScope: 'gateway',
    },
    {
      key: 'can_fork',
      labelKey: 'settings.allowForking',
      descriptionKey: 'settings.allowForkingDescription',
      type: 'toggle',
      hotReload: false,
      restartScope: 'gateway',
    },
    {
      key: 'max_execution_time_secs',
      labelKey: 'settings.maxExecutionTimeS',
      descriptionKey: 'settings.maxExecutionTimeSDescription',
      type: 'range',
      min: 30,
      max: 3600,
      step: 30,
      placeholder: '300',
      hotReload: false,
      restartScope: 'gateway',
    },
    {
      key: 'max_memory_mb',
      labelKey: 'settings.maxMemoryMB',
      descriptionKey: 'settings.maxMemoryMBDescription',
      type: 'range',
      min: 64,
      max: 4096,
      step: 64,
      placeholder: '512',
      hotReload: false,
      restartScope: 'gateway',
    },
    {
      key: 'max_audit_entries',
      labelKey: 'settings.maxAuditEntries',
      descriptionKey: 'settings.maxAuditEntriesDescription',
      type: 'number',
      placeholder: '10000',
      hotReload: false,
      restartScope: 'audit',
    },
    {
      key: 'audit_log_path',
      labelKey: 'settings.auditLogPath',
      descriptionKey: 'settings.auditLogPathDescription',
      type: 'text',
      placeholder: '~/.oxios/audit.log',
      hotReload: false,
      restartScope: 'audit',
    },
    {
      key: 'rate_limit_per_minute',
      labelKey: 'settings.rateLimitPerMinute',
      descriptionKey: 'settings.rateLimitPerMinuteDescription',
      type: 'range',
      min: 10,
      max: 300,
      step: 10,
      placeholder: '120',
      hotReload: false,
      restartScope: 'gateway',
    },
  ],
}

// ---------------------------------------------------------------------------
// 3. memory — Memory (storage + embedding + learning + dream)
// ---------------------------------------------------------------------------

const memorySection: SettingsSectionDef = {
  key: 'memory',
  labelKey: 'settings.memory',
  descriptionKey: 'settings.memoryDescription',
  iconKey: 'memory',
  groupId: 'memory',
  fields: [
    // The memory subsystem is constructed at boot (SQLite handle,
    // embedding model, SONA state). Toggling `enabled` via PATCH
    // persists the new value but does not construct/teardown the
    // subsystem at runtime. Restart is required to apply.
    {
      key: 'enabled',
      labelKey: 'settings.memoryEnabled',
      descriptionKey: 'settings.memoryEnabledDescription',
      type: 'toggle',
      hotReload: false,
      restartScope: 'memory',
    },
    {
      key: 'sqlite.path',
      labelKey: 'settings.memoryStoragePath',
      descriptionKey: 'settings.memoryStoragePathDescription',
      type: 'text',
      placeholder: '~/.oxios/workspace/memory.db',
      hotReload: false,
      restartScope: 'memory',
      dependsOn: { field: 'enabled', value: true },
    },
    {
      key: 'embedding.provider',
      labelKey: 'settings.embeddingProvider',
      descriptionKey: 'settings.embeddingProviderDescription',
      type: 'select',
      options: [
        { value: 'gguf', labelKey: 'settings.embeddingProviderGguf' },
        { value: 'mlx', labelKey: 'settings.embeddingProviderMlx' },
        { value: 'tfidf', labelKey: 'settings.embeddingProviderTfidf' },
      ],
      hotReload: false,
      restartScope: 'memory',
      dependsOn: { field: 'enabled', value: true },
    },
    {
      key: 'learning.enabled',
      labelKey: 'settings.sonaEnabled',
      descriptionKey: 'settings.sonaEnabledDescription',
      type: 'toggle',
      hotReload: false,
      restartScope: 'memory',
      dependsOn: { field: 'enabled', value: true },
    },
    {
      key: 'consolidation.preset',
      labelKey: 'settings.consolidationPreset',
      descriptionKey: 'settings.consolidationPresetDescription',
      type: 'select',
      options: [
        { value: 'conservative', labelKey: 'settings.presetConservative' },
        { value: 'balanced', labelKey: 'settings.presetBalanced' },
        { value: 'aggressive', labelKey: 'settings.presetAggressive' },
        { value: 'custom', labelKey: 'settings.presetCustom' },
      ],
      hotReload: false,
      restartScope: 'memory',
      dependsOn: { field: 'enabled', value: true },
    },
    {
      key: 'consolidation.dream_enabled',
      labelKey: 'settings.dreamEnabled',
      descriptionKey: 'settings.dreamEnabledDescription',
      type: 'toggle',
      hotReload: false,
      restartScope: 'memory',
      dependsOn: { field: 'enabled', value: true },
    },
    {
      key: 'consolidation.dream_interval_hours',
      labelKey: 'settings.dreamIntervalHours',
      descriptionKey: 'settings.dreamIntervalHoursDescription',
      type: 'range',
      min: 1,
      max: 72,
      placeholder: '24',
      hotReload: false,
      restartScope: 'memory',
      dependsOn: { field: 'consolidation.dream_enabled', value: true },
    },
  ],
}

// ---------------------------------------------------------------------------
// 4. channels.telegram — Telegram channel
// ---------------------------------------------------------------------------

// Field keys here are the path *below* `channels.telegram`. The section
// key (`channels.telegram`) is prepended by `buildPayload` (see
// `routes/settings.tsx`), matching how the `memory` section encodes
// sub-paths like `embedding.provider` under the `memory` section key.
// Do not include the `channels.telegram.` prefix in `field.key` — the
// payload builder would double-nest the change and the user's edit
// would land at `config.channels.telegram.channels.telegram.*`, which
// `OxiosConfig` deserialization silently drops.
const telegramSection: SettingsSectionDef = {
  key: 'channels.telegram',
  labelKey: 'settings.telegram',
  descriptionKey: 'settings.telegramDescription',
  iconKey: 'channels',
  groupId: 'channels',
  fields: [
    {
      key: 'bot_token_env',
      labelKey: 'settings.telegramBotTokenEnv',
      descriptionKey: 'settings.telegramBotTokenEnvDescription',
      type: 'text',
      placeholder: 'TELEGRAM_BOT_TOKEN',
      hotReload: false,
      restartScope: 'gateway',
    },
    {
      key: 'allowed_users',
      labelKey: 'settings.telegramAllowedUsers',
      descriptionKey: 'settings.telegramAllowedUsersDescription',
      type: 'numbers',
      placeholder: '123456789',
      hotReload: false,
      restartScope: 'gateway',
    },
    {
      key: 'session.rotation_hours',
      labelKey: 'settings.telegramSessionRotationHours',
      descriptionKey: 'settings.telegramSessionRotationHoursDescription',
      type: 'range',
      min: 1,
      max: 48,
      placeholder: '2',
      hotReload: false,
      restartScope: 'gateway',
    },
    {
      key: 'session.max_messages',
      labelKey: 'settings.telegramSessionMaxMessages',
      descriptionKey: 'settings.telegramSessionMaxMessagesDescription',
      type: 'number',
      placeholder: '0',
      hotReload: false,
      restartScope: 'gateway',
    },
  ],
}

// ---------------------------------------------------------------------------
// 5. audit — Audit trail
// ---------------------------------------------------------------------------

// Audit trail writer is constructed at boot with its rotating file
// handle and ring-buffer capacity. Changing `enabled` or `max_entries`
// persists but does not re-open the writer; restart is required.
const auditSection: SettingsSectionDef = {
  key: 'audit',
  labelKey: 'settings.audit',
  descriptionKey: 'settings.auditDescription',
  iconKey: 'audit',
  groupId: 'security',
  fields: [
    {
      key: 'enabled',
      labelKey: 'settings.auditEnabled',
      descriptionKey: 'settings.auditEnabledDescription',
      type: 'toggle',
      hotReload: false,
      restartScope: 'audit',
    },
    {
      key: 'max_entries',
      labelKey: 'settings.auditMaxEntries',
      descriptionKey: 'settings.auditMaxEntriesDescription',
      type: 'number',
      placeholder: '100000',
      hotReload: false,
      restartScope: 'audit',
      dependsOn: { field: 'enabled', value: true },
    },
  ],
}

// ---------------------------------------------------------------------------
// 6. calendar — Calendar configuration (RFC-028 SP-2a)
// ---------------------------------------------------------------------------

const calendarSection: SettingsSectionDef = {
  key: 'calendar',
  labelKey: 'settings.sectionCalendar',
  descriptionKey: 'settings.calendarDescription',
  iconKey: 'calendar',
  groupId: 'channels',
  fields: [
    {
      key: 'enabled',
      labelKey: 'settings.calendarEnabled',
      descriptionKey: 'settings.calendarEnabledDesc',
      type: 'toggle',
      hotReload: false,
    },
    {
      key: 'timezone',
      labelKey: 'settings.calendarTimezone',
      descriptionKey: 'settings.calendarTimezoneDesc',
      type: 'text',
      placeholder: 'Asia/Seoul',
      hotReload: false,
    },
    {
      key: 'default_reminder_minutes',
      labelKey: 'settings.calendarReminders',
      descriptionKey: 'settings.calendarRemindersDesc',
      type: 'numbers',
      hotReload: false,
    },
    {
      key: 'alarm_channels',
      labelKey: 'settings.calendarAlarmChannels',
      descriptionKey: 'settings.calendarAlarmChannelsDesc',
      type: 'tags',
      hotReload: false,
    },
    {
      key: 'journal_sync',
      labelKey: 'settings.calendarJournalSync',
      descriptionKey: 'settings.calendarJournalSyncDesc',
      type: 'select',
      options: [
        { value: 'on_open', labelKey: 'settings.calendarJournalOnOpen' },
        { value: 'midnight', labelKey: 'settings.calendarJournalMidnight' },
        { value: 'both', labelKey: 'settings.calendarJournalBoth' },
      ],
      hotReload: false,
    },
    {
      key: 'system_calendar',
      labelKey: 'settings.calendarSystemCalendar',
      descriptionKey: 'settings.calendarSystemCalendarDesc',
      type: 'toggle',
      hotReload: false,
    },
    {
      key: 'archive_after_days',
      labelKey: 'settings.calendarArchiveDays',
      descriptionKey: 'settings.calendarArchiveDaysDesc',
      type: 'number',
      hotReload: false,
    },
  ],
}

// ---------------------------------------------------------------------------
// 7. otel — OpenTelemetry tracing
// ---------------------------------------------------------------------------

const otelSection: SettingsSectionDef = {
  key: 'otel',
  labelKey: 'settings.sectionOtel',
  descriptionKey: 'settings.otelDescription',
  iconKey: 'otel',
  groupId: 'system',
  fields: [
    {
      key: 'enabled',
      labelKey: 'settings.otelEnabled',
      descriptionKey: 'settings.otelEnabledDesc',
      type: 'toggle',
      hotReload: false,
    },
    {
      key: 'endpoint',
      labelKey: 'settings.otelEndpoint',
      descriptionKey: 'settings.otelEndpointDesc',
      type: 'text',
      placeholder: 'http://localhost:4317',
      hotReload: false,
    },
    {
      key: 'service_name',
      labelKey: 'settings.otelServiceName',
      descriptionKey: 'settings.otelServiceNameDesc',
      type: 'text',
      placeholder: 'oxios',
      hotReload: false,
    },
    {
      key: 'sampling_ratio',
      labelKey: 'settings.otelSamplingRatio',
      descriptionKey: 'settings.otelSamplingRatioDesc',
      type: 'range',
      min: 0,
      max: 1,
      step: 0.1,
      hotReload: false,
    },
  ],
}

// ---------------------------------------------------------------------------
// 8. agent_log — Agent history log
// ---------------------------------------------------------------------------

const agentLogSection: SettingsSectionDef = {
  key: 'agent_log',
  labelKey: 'settings.sectionAgentLog',
  descriptionKey: 'settings.agentLogDescription',
  iconKey: 'agentLog',
  groupId: 'system',
  fields: [
    {
      key: 'max_entries',
      labelKey: 'settings.agentLogMaxEntries',
      descriptionKey: 'settings.agentLogMaxEntriesDesc',
      type: 'number',
      hotReload: false,
    },
    {
      key: 'ttl_hours',
      labelKey: 'settings.agentLogTtlHours',
      descriptionKey: 'settings.agentLogTtlHoursDesc',
      type: 'number',
      hotReload: false,
    },
    {
      key: 'max_tool_calls_per_agent',
      labelKey: 'settings.agentLogMaxToolCalls',
      descriptionKey: 'settings.agentLogMaxToolCallsDesc',
      type: 'number',
      hotReload: false,
    },
    {
      key: 'prune_batch_size',
      labelKey: 'settings.agentLogPruneBatch',
      descriptionKey: 'settings.agentLogPruneBatchDesc',
      type: 'number',
      hotReload: false,
    },
    {
      key: 'db_path',
      labelKey: 'settings.agentLogDbPath',
      descriptionKey: 'settings.agentLogDbPathDesc',
      type: 'text',
      placeholder: '(default location)',
      hotReload: false,
    },
  ],
}

// ---------------------------------------------------------------------------
// 9. resource_monitor — System resource monitoring
// ---------------------------------------------------------------------------

const resourceMonitorSection: SettingsSectionDef = {
  key: 'resource_monitor',
  labelKey: 'settings.sectionResourceMonitor',
  descriptionKey: 'settings.resourceMonitorDescription',
  iconKey: 'resourceMonitor',
  groupId: 'system',
  fields: [
    {
      key: 'interval_secs',
      labelKey: 'settings.rmInterval',
      descriptionKey: 'settings.rmIntervalDesc',
      type: 'number',
      hotReload: false,
    },
    {
      key: 'history_max',
      labelKey: 'settings.rmHistoryMax',
      descriptionKey: 'settings.rmHistoryMaxDesc',
      type: 'number',
      hotReload: false,
    },
    {
      key: 'cpu_threshold',
      labelKey: 'settings.rmCpuThreshold',
      descriptionKey: 'settings.rmCpuThresholdDesc',
      type: 'range',
      min: 0,
      max: 100,
      step: 1,
      hotReload: false,
    },
    {
      key: 'memory_threshold',
      labelKey: 'settings.rmMemThreshold',
      descriptionKey: 'settings.rmMemThresholdDesc',
      type: 'range',
      min: 0,
      max: 100,
      step: 1,
      hotReload: false,
    },
    {
      key: 'load_threshold',
      labelKey: 'settings.rmLoadThreshold',
      descriptionKey: 'settings.rmLoadThresholdDesc',
      type: 'number',
      hotReload: false,
    },
  ],
}

// ---------------------------------------------------------------------------
// 10. browser — Headless browser integration
// ---------------------------------------------------------------------------

const browserSection: SettingsSectionDef = {
  key: 'browser',
  labelKey: 'settings.sectionBrowser',
  descriptionKey: 'settings.browserDescription',
  iconKey: 'browser',
  groupId: 'system',
  fields: [
    {
      key: 'enabled',
      labelKey: 'settings.browserEnabled',
      descriptionKey: 'settings.browserEnabledDesc',
      type: 'toggle',
      hotReload: false,
    },
    {
      key: 'engine',
      labelKey: 'settings.browserEngine',
      descriptionKey: 'settings.browserEngineDesc',
      type: 'multiline',
      placeholder: '{\n  "user_agent": "MyBot/1.0"\n}',
      hotReload: false,
    },
  ],
}

// ---------------------------------------------------------------------------
// 11. budget — Budget enforcement
// ---------------------------------------------------------------------------

const budgetSection: SettingsSectionDef = {
  key: 'budget',
  labelKey: 'settings.sectionBudget',
  descriptionKey: 'settings.budgetDescription',
  iconKey: 'budget',
  groupId: 'system',
  fields: [
    {
      key: 'enabled',
      labelKey: 'settings.budgetEnabled',
      descriptionKey: 'settings.budgetEnabledDesc',
      type: 'toggle',
      hotReload: false,
    },
    {
      key: 'default_token_budget',
      labelKey: 'settings.budgetTokenBudget',
      descriptionKey: 'settings.budgetTokenBudgetDesc',
      type: 'number',
      hotReload: false,
    },
    {
      key: 'default_calls_budget',
      labelKey: 'settings.budgetCallsBudget',
      descriptionKey: 'settings.budgetCallsBudgetDesc',
      type: 'number',
      hotReload: false,
    },
    {
      key: 'default_window_secs',
      labelKey: 'settings.budgetWindowSecs',
      descriptionKey: 'settings.budgetWindowSecsDesc',
      type: 'number',
      hotReload: false,
    },
  ],
}

// ---------------------------------------------------------------------------
// All new sections (MVP)
// ---------------------------------------------------------------------------

export const NEW_SECTIONS: SettingsSectionDef[] = [
  execSection,
  securitySection,
  memorySection,
  telegramSection,
  auditSection,
  calendarSection,
  otelSection,
  agentLogSection,
  resourceMonitorSection,
  browserSection,
  budgetSection,
]

// ---------------------------------------------------------------------------
// Group definitions for the left sidebar
// ---------------------------------------------------------------------------

export interface SettingsGroup {
  id: 'ai' | 'system' | 'security' | 'memory' | 'channels'
  labelKey: string
  sectionKeys: string[]
}

export const SETTINGS_GROUPS: SettingsGroup[] = [
  {
    id: 'ai',
    labelKey: 'settings.groupAi',
    sectionKeys: ['engine'],
  },
  {
    id: 'system',
    labelKey: 'settings.groupSystem',
    sectionKeys: [
      'kernel',
      'exec',
      'scheduler',
      'orchestrator',
      'context',
      'gateway',
      'session',
      'logging',
      'update',
      'otel',
      'agent_log',
      'resource_monitor',
      'browser',
      'budget',
      'notifications',
    ],
  },
  {
    id: 'security',
    labelKey: 'settings.groupSecurity',
    sectionKeys: ['security', 'audit', 'secrets'],
  },
  {
    id: 'memory',
    labelKey: 'settings.groupMemory',
    sectionKeys: ['memory'],
  },
  {
    id: 'channels',
    labelKey: 'settings.groupChannels',
    sectionKeys: ['channels.telegram', 'calendar'],
  },
]

// ---------------------------------------------------------------------------
// Lookup helpers
// ---------------------------------------------------------------------------

export const newSectionsByKey = new Map(NEW_SECTIONS.map((s) => [s.key, s]))

/** Returns the field def for a given section + dotted field key. */
export function findFieldDef(sectionKey: string, fieldKey: string): SettingsFieldDef | undefined {
  return newSectionsByKey.get(sectionKey)?.fields.find((f) => f.key === fieldKey)
}

/**
 * Builds a lookup from full dotted config path (`sectionKey.field.key`)
 * to the i18n label key, for every field in `NEW_SECTIONS`.
 *
 * Used by the diff-preview dialog to show a human-readable label
 * instead of the raw config path (e.g. `memory.learning.sona_enabled`
 * → `settings.sonaEnabled`).
 */
export const pathLabelMap: Map<string, string> = (() => {
  const m = new Map<string, string>()
  for (const section of NEW_SECTIONS) {
    for (const field of section.fields) {
      m.set(`${section.key}.${field.key}`, field.labelKey)
    }
  }
  return m
})()

// ---------------------------------------------------------------------------
// Unified section metadata
// ---------------------------------------------------------------------------
//
// `NEW_SECTIONS` covers the sections with the new declarative
// field-rendering model. Older sections (`engine`, `kernel`, `scheduler`,
// `orchestrator`, `context`, `gateway`, `session`, `logging`, `update`)
// are still rendered with their custom components. To build the left
// rail and section tabs from a single source of truth, we list their
// metadata here.

export type SectionIconKey =
  | 'engine'
  | 'kernel'
  | 'exec'
  | 'security'
  | 'scheduler'
  | 'orchestrator'
  | 'context'
  | 'gateway'
  | 'session'
  | 'logging'
  | 'memory'
  | 'channels'
  | 'audit'
  | 'update'
  | 'calendar'
  | 'otel'
  | 'agentLog'
  | 'resourceMonitor'
  | 'browser'
  | 'budget'
  | 'secrets'
  | 'notifications'

export interface SectionMeta {
  /** Section key, e.g. `engine`, `exec`, `memory`. */
  id: string
  /** i18n key for the section title. */
  labelKey: string
  /** i18n key for the section description (used by the rail/section card). */
  descriptionKey: string
  /** Group id for rail grouping. Must match a group in SETTINGS_GROUPS. */
  groupId: 'ai' | 'system' | 'security' | 'memory' | 'channels'
  /** Icon key for the rail/section card icon. */
  iconKey: SectionIconKey
  /** Whether this section renders its own custom component (EnginePanel, SystemUpdateCard, etc.). */
  custom: boolean
}

export const SECTION_META: SectionMeta[] = [
  // AI
  {
    id: 'engine',
    labelKey: 'settings.sectionEngine',
    descriptionKey: 'settings.engineDescription',
    groupId: 'ai',
    iconKey: 'engine',
    custom: true,
  },
  // System
  {
    id: 'kernel',
    labelKey: 'settings.sectionKernel',
    descriptionKey: 'settings.kernelDescription',
    groupId: 'system',
    iconKey: 'kernel',
    custom: false,
  },
  {
    id: 'exec',
    labelKey: 'settings.sectionExec',
    descriptionKey: 'settings.executionDescription',
    groupId: 'system',
    iconKey: 'exec',
    custom: false,
  },
  {
    id: 'scheduler',
    labelKey: 'settings.sectionScheduler',
    descriptionKey: 'settings.schedulerDescription',
    groupId: 'system',
    iconKey: 'scheduler',
    custom: false,
  },
  {
    id: 'orchestrator',
    labelKey: 'settings.sectionOrchestrator',
    descriptionKey: 'settings.orchestratorDescription',
    groupId: 'system',
    iconKey: 'orchestrator',
    custom: false,
  },
  {
    id: 'context',
    labelKey: 'settings.sectionContext',
    descriptionKey: 'settings.contextDescription',
    groupId: 'system',
    iconKey: 'context',
    custom: false,
  },
  {
    id: 'gateway',
    labelKey: 'settings.sectionGateway',
    descriptionKey: 'settings.gatewayDescription',
    groupId: 'system',
    iconKey: 'gateway',
    custom: false,
  },
  {
    id: 'session',
    labelKey: 'settings.sectionSession',
    descriptionKey: 'settings.sessionDescription',
    groupId: 'system',
    iconKey: 'session',
    custom: false,
  },
  {
    id: 'logging',
    labelKey: 'settings.sectionLogging',
    descriptionKey: 'settings.loggingDescription',
    groupId: 'system',
    iconKey: 'logging',
    custom: false,
  },
  {
    id: 'update',
    labelKey: 'settings.update',
    descriptionKey: 'settings.updateDescription',
    groupId: 'system',
    iconKey: 'update',
    custom: true,
  },
  // Security
  {
    id: 'security',
    labelKey: 'settings.sectionSecurity',
    descriptionKey: 'settings.securityDescription',
    groupId: 'security',
    iconKey: 'security',
    custom: false,
  },
  {
    id: 'audit',
    labelKey: 'settings.sectionAudit',
    descriptionKey: 'settings.auditDescription',
    groupId: 'security',
    iconKey: 'audit',
    custom: false,
  },
  // Memory
  {
    id: 'memory',
    labelKey: 'settings.sectionMemory',
    descriptionKey: 'settings.memoryDescription',
    groupId: 'memory',
    iconKey: 'memory',
    custom: false,
  },
  // Channels
  {
    id: 'channels.telegram',
    labelKey: 'settings.sectionTelegram',
    descriptionKey: 'settings.telegramDescription',
    groupId: 'channels',
    iconKey: 'channels',
    custom: false,
  },
  // RFC-028 SP-2a: declarative config sections
  {
    id: 'calendar',
    labelKey: 'settings.sectionCalendar',
    descriptionKey: 'settings.calendarDescription',
    groupId: 'channels',
    iconKey: 'calendar',
    custom: false,
  },
  {
    id: 'otel',
    labelKey: 'settings.sectionOtel',
    descriptionKey: 'settings.otelDescription',
    groupId: 'system',
    iconKey: 'otel',
    custom: false,
  },
  {
    id: 'agent_log',
    labelKey: 'settings.sectionAgentLog',
    descriptionKey: 'settings.agentLogDescription',
    groupId: 'system',
    iconKey: 'agentLog',
    custom: false,
  },
  {
    id: 'resource_monitor',
    labelKey: 'settings.sectionResourceMonitor',
    descriptionKey: 'settings.resourceMonitorDescription',
    groupId: 'system',
    iconKey: 'resourceMonitor',
    custom: false,
  },
  {
    id: 'browser',
    labelKey: 'settings.sectionBrowser',
    descriptionKey: 'settings.browserDescription',
    groupId: 'system',
    iconKey: 'browser',
    custom: false,
  },
  {
    id: 'budget',
    labelKey: 'settings.sectionBudget',
    descriptionKey: 'settings.budgetDescription',
    groupId: 'system',
    iconKey: 'budget',
    custom: false,
  },
  // RFC-028 SP-2c / SP-1e: custom-rendered sections
  {
    id: 'secrets',
    labelKey: 'settings.sectionSecrets',
    descriptionKey: 'settings.secretsDescription',
    groupId: 'security',
    iconKey: 'secrets',
    custom: true,
  },
  {
    id: 'notifications',
    labelKey: 'settings.sectionNotifications',
    descriptionKey: 'settings.notificationsDescription',
    groupId: 'system',
    iconKey: 'notifications',
    custom: true,
  },
]

const sectionMetaById = new Map(SECTION_META.map((m) => [m.id, m]))

/** Lookup helper. Returns `undefined` for unknown ids. */
export function getSectionMeta(id: string): SectionMeta | undefined {
  return sectionMetaById.get(id)
}
