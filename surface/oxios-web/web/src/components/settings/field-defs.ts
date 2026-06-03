// Field definitions for the Settings UI.
//
// Each entry is a triple:
//   [sectionKey, descriptionKey, fields[]]
//
// `field.hotReload` mirrors the backend classification in
// `surface/oxios-web/src/routes/system.rs::is_restart_required`. The two
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
  /** If false, the field requires a daemon restart to take effect. */
  hotReload: boolean
  /** Sub-system that consumes this value (used in tooltips). */
  restartScope?: 'kernel' | 'gateway' | 'logging' | 'memory' | 'engine' | 'audit'
}

export interface SettingsSectionDef {
  key: string
  labelKey: string
  descriptionKey: string
  iconKey:
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
  /** Sub-section used by `SettingsLayout` group navigation. */
  groupId: 'ai' | 'system' | 'security' | 'memory' | 'channels' | 'advanced'
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
      type: 'number',
      placeholder: '120',
      hotReload: true,
    },
    {
      key: 'max_timeout_secs',
      labelKey: 'settings.maxTimeoutS',
      descriptionKey: 'settings.maxTimeoutSDescription',
      type: 'number',
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
      hotReload: true,
      restartScope: 'gateway',
    },
    {
      key: 'allowed_tools',
      labelKey: 'settings.allowedTools',
      descriptionKey: 'settings.allowedToolsDescription',
      type: 'csv',
      placeholder: 'read, write, edit, bash',
      hotReload: true,
    },
    {
      key: 'cors_origins',
      labelKey: 'settings.corsOrigins',
      descriptionKey: 'settings.corsOriginsDescription',
      type: 'csv',
      placeholder: 'http://localhost:4200, http://localhost:3000',
      hotReload: true,
    },
    {
      key: 'network_access',
      labelKey: 'settings.networkAccess',
      descriptionKey: 'settings.networkAccessDescription',
      type: 'toggle',
      hotReload: true,
    },
    {
      key: 'can_fork',
      labelKey: 'settings.allowForking',
      descriptionKey: 'settings.allowForkingDescription',
      type: 'toggle',
      hotReload: true,
    },
    {
      key: 'max_execution_time_secs',
      labelKey: 'settings.maxExecutionTimeS',
      descriptionKey: 'settings.maxExecutionTimeSDescription',
      type: 'number',
      placeholder: '300',
      hotReload: true,
    },
    {
      key: 'max_memory_mb',
      labelKey: 'settings.maxMemoryMB',
      descriptionKey: 'settings.maxMemoryMBDescription',
      type: 'number',
      placeholder: '512',
      hotReload: true,
    },
    {
      key: 'max_audit_entries',
      labelKey: 'settings.maxAuditEntries',
      descriptionKey: 'settings.maxAuditEntriesDescription',
      type: 'number',
      placeholder: '10000',
      hotReload: true,
    },
    {
      key: 'audit_log_path',
      labelKey: 'settings.auditLogPath',
      descriptionKey: 'settings.auditLogPathDescription',
      type: 'text',
      placeholder: '~/.oxios/audit.log',
      hotReload: true,
    },
    {
      key: 'rate_limit_per_minute',
      labelKey: 'settings.rateLimitPerMinute',
      descriptionKey: 'settings.rateLimitPerMinuteDescription',
      type: 'number',
      placeholder: '120',
      hotReload: true,
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
    { key: 'enabled', labelKey: 'settings.memoryEnabled', descriptionKey: 'settings.memoryEnabledDescription', type: 'toggle', hotReload: true },
    {
      key: 'sqlite.path',
      labelKey: 'settings.memoryStoragePath',
      descriptionKey: 'settings.memoryStoragePathDescription',
      type: 'text',
      placeholder: '~/.oxios/workspace/memory.db',
      hotReload: false,
      restartScope: 'memory',
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
    },
    {
      key: 'learning.sona_enabled',
      labelKey: 'settings.sonaEnabled',
      descriptionKey: 'settings.sonaEnabledDescription',
      type: 'toggle',
      hotReload: true,
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
    },
    {
      key: 'consolidation.dream_enabled',
      labelKey: 'settings.dreamEnabled',
      descriptionKey: 'settings.dreamEnabledDescription',
      type: 'toggle',
      hotReload: true,
    },
    {
      key: 'consolidation.dream_interval_hours',
      labelKey: 'settings.dreamIntervalHours',
      descriptionKey: 'settings.dreamIntervalHoursDescription',
      type: 'number',
      placeholder: '24',
      hotReload: true,
    },
  ],
}

// ---------------------------------------------------------------------------
// 4. channels.telegram — Telegram channel
// ---------------------------------------------------------------------------

const telegramSection: SettingsSectionDef = {
  key: 'channels.telegram',
  labelKey: 'settings.telegram',
  descriptionKey: 'settings.telegramDescription',
  iconKey: 'channels',
  groupId: 'channels',
  fields: [
    {
      key: 'channels.telegram.bot_token_env',
      labelKey: 'settings.telegramBotTokenEnv',
      descriptionKey: 'settings.telegramBotTokenEnvDescription',
      type: 'text',
      placeholder: 'TELEGRAM_BOT_TOKEN',
      hotReload: false,
      restartScope: 'gateway',
    },
    {
      key: 'channels.telegram.allowed_users',
      labelKey: 'settings.telegramAllowedUsers',
      descriptionKey: 'settings.telegramAllowedUsersDescription',
      type: 'numbers',
      placeholder: '123456789',
      hotReload: false,
      restartScope: 'gateway',
    },
    {
      key: 'channels.telegram.session.rotation_hours',
      labelKey: 'settings.telegramSessionRotationHours',
      descriptionKey: 'settings.telegramSessionRotationHoursDescription',
      type: 'number',
      placeholder: '2',
      hotReload: false,
      restartScope: 'gateway',
    },
    {
      key: 'channels.telegram.session.max_messages',
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
      hotReload: true,
    },
    {
      key: 'max_entries',
      labelKey: 'settings.auditMaxEntries',
      descriptionKey: 'settings.auditMaxEntriesDescription',
      type: 'number',
      placeholder: '100000',
      hotReload: true,
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
]

// ---------------------------------------------------------------------------
// Group definitions for the left sidebar
// ---------------------------------------------------------------------------

export interface SettingsGroup {
  id: 'ai' | 'system' | 'security' | 'memory' | 'channels' | 'advanced'
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
      'exec', // exec is moved into System per RFC (allowlist is system-wide)
      'scheduler',
      'orchestrator',
      'context',
      'gateway',
      'session',
      'logging',
      'update',
    ],
  },
  {
    id: 'security',
    labelKey: 'settings.groupSecurity',
    sectionKeys: ['security', 'audit'],
  },
  {
    id: 'memory',
    labelKey: 'settings.groupMemory',
    sectionKeys: ['memory'],
  },
  {
    id: 'channels',
    labelKey: 'settings.groupChannels',
    sectionKeys: ['channels.telegram'],
  },
  {
    id: 'advanced',
    labelKey: 'settings.groupAdvanced',
    sectionKeys: ['resource_monitor', 'otel', 'daemon', 'persona', 'cron', 'mcp', 'browser', 'marketplace'],
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
