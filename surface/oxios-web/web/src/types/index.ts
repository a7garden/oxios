// Agent
export interface Agent {
  id: string
  name: string
  status: string // Backend sends Debug format: "Running", "Idle", "Stopped", "Starting", "Error"
  created_at?: string
  seed_id?: string
}

export interface AgentListResponse {
  items: Agent[]
  total: number
  page: number
  limit: number
}

// Session
export interface Session {
  id: string
  user_id?: string
  project_id?: string
  active_seed_id?: string
  created_at: string
  updated_at?: string
  message_count?: number
  metadata?: Record<string, unknown>
}

// Session detail (from GET /api/sessions/:id)
export interface SessionDetail {
  id: string
  user_id: string
  project_id?: string
  user_messages: { content: string; timestamp: string }[]
  agent_responses: {
    content: string
    session_id: string
    seed_id: string
    phase_reached: string
    evaluation_passed: boolean
    timestamp: string
  }[]
  active_seed_id?: string
  active_persona_id?: string
  created_at: string
  updated_at: string
  metadata?: Record<string, unknown>
}

// Seed
export interface Seed {
  id: string
  goal: string
  constraints_count: number
  created_at: string
}

export interface EvolutionEntry {
  phase: string
  timestamp: string
  summary: string
  changes?: Record<string, unknown>
}

// Project (replaces Space)
export interface Project {
  id: string
  name: string
  description?: string
  paths?: string[]
  tags?: string[]
  emoji?: string
  source?: string
  memory_visible?: boolean
  created_at: string
  updated_at?: string
  last_active_at?: string
  metadata?: Record<string, unknown>
}

// Skill (RFC-009 unified model)
export type SkillSource = 'bundled' | 'managed' | 'workspace'
export type SkillStatus = 'ready' | 'needs_setup' | 'disabled'
export type SkillFormat = 'oxios' | 'openclaw' | 'claude_code' | 'agent_skills'

export interface SkillRequirements {
  bins: string[]
  anyBins: string[]
  env: string[]
  config: string[]
}

export interface SkillInstallSpec {
  kind: 'brew' | 'node' | 'go' | 'uv' | 'download'
  label?: string
  bins: string[]
}

export interface Skill {
  name: string
  description: string
  author?: string
  version?: string
  emoji?: string
  homepage?: string
  source: SkillSource
  bundled: boolean
  status: SkillStatus
  eligible: boolean
  always: boolean
  user_invocable: boolean
  file_path: string
  requirements: SkillRequirements
  missing: SkillRequirements
  os: string[]
  install: SkillInstallSpec[]
  config_checks: Array<{ path: string; satisfied: boolean }>
  format: SkillFormat
}

// Memory
export interface MemoryEntry {
  name: string
  category?: string
  content?: string
  created_at?: string
  updated_at?: string
}

// Config
export interface OxiosConfig {
  general?: {
    default_model?: string
    max_concurrent_agents?: number
    workspace_path?: string
    [key: string]: unknown
  }
  engine?: {
    provider?: string
    model?: string
    api_key?: string
    base_url?: string
    [key: string]: unknown
  }
  [key: string]: unknown
}

// Chat
export interface ChatMessage {
  id: string
  role: 'user' | 'assistant' | 'system' | 'tool'
  content: string
  timestamp?: string
  // Tool call fields (role === 'tool')
  toolName?: string
  toolArgs?: Record<string, unknown>
  toolResult?: unknown
  toolDurationMs?: number
  // Completion metadata (last assistant message)
  metadata?: {
    phase?: string
    evaluation_passed?: boolean
    seed_id?: string
    duration_ms?: number
    tool_calls?: ToolCallSummary[]
  }
  // RFC-015: real-time activity timeline attached to an assistant turn.
  // Populated as the agent executes tools, recalls memories, emits reasoning
  // fragments, and reports token usage. Also restored from the persisted
  // trajectory_steps when a session is re-opened.
  activities?: ChatActivity[]
  // RFC-015: cumulative token usage for the turn.
  totalInputTokens?: number
  totalOutputTokens?: number
  // Interview history: persisted questions from a completed interview round.
  // Used by MessageBubble to render the Q&A exchange inline. Prefixed with
  // _ to signal this is internal-only and not serialized to the backend.
  _interviewQuestions?: InterviewQuestion[]
  _interviewRound?: number
}

// RFC-015: a single transparency activity entry shown in the chat timeline.
export type ChatActivityType = 'phase' | 'tool_call' | 'memory' | 'reasoning' | 'usage'

export interface ChatActivity {
  id: string
  type: ChatActivityType
  timestamp: string
  // phase
  phase?: string
  status?: 'started' | 'completed'
  summary?: string
  // tool_call
  toolName?: string
  toolCallId?: string
  toolArgs?: Record<string, unknown>
  outputSummary?: string
  durationMs?: number
  isError?: boolean
  /// Latest progress text from a running tool (RFC-015 v0.12). Replaced
  /// in place as the tool emits new updates.
  progress?: string
  /// True while a tool is still running. Drives the spinner in
  /// `ActivityCard` and is cleared on `tool_end`.
  isRunning?: boolean
  /// Browser tab id that produced this tool call (when the upstream tool
  /// is tab-aware, e.g. browser). Rendered as a short badge in
  /// `ActivityCard` so users can distinguish concurrent tab activity.
  tabId?: string
  /// Semantic context from browsing tool (PageVisit, WebSearch, etc.).
  /// Used by `BrowseContextBadge` / `BrowseContextDetail` for rich rendering.
  context?: ToolCallContext
  // memory
  memoryAction?: 'recall' | 'store'
  query?: string
  count?: number
  memorySource?: string
  // reasoning
  content?: string
  reasoningSource?: string
  // usage
  inputTokens?: number
  outputTokens?: number
}

export interface ToolCallSummary {
  tool_name: string
  input: string
  output: string
  duration_ms: number
}

export interface ChatRequest {
  message: string
  session_id?: string
  project_id?: string
}

export interface ChatResponse {
  response: string
  session_id: string
  project_id?: string
  seed_id?: string
  agent_id?: string
  phase_reached?: string
  evaluation_passed?: boolean
  exit_code?: number
  duration_ms?: number
}

// ── Interactive interview (chat UI redesign) ────────────────────────────

export interface InterviewOption {
  value: string
  label: string
  description?: string
}

export interface InterviewQuestion {
  id: string
  text: string
  kind: 'single_choice' | 'multi_choice' | 'free_text' | 'yes_no'
  options?: InterviewOption[]
}

export interface InterviewAnswer {
  question_id: string
  value: string
}

export interface StreamChunk {
  type:
    | 'token'
    | 'tool_call'
    | 'tool_result'
    | 'done'
    | 'error'
    // RFC-015 chat transparency chunks
    | 'phase'
    | 'tool_start'
    | 'tool_end'
    | 'tool_progress'
    | 'memory'
    | 'reasoning'
    | 'usage'
    // Chat UI redesign: interactive interview
    | 'interview'
    // RFC-017: runtime tool capability escalation
    | 'tool_approval'
  content?: string
  tool_name?: string
  tool_args?: Record<string, unknown>
  tool_result?: unknown
  error?: string
  session_id?: string
  project_id?: string
  phase?: string
  evaluation_passed?: boolean | string
  seed_id?: string
  duration_ms?: number
  tool_calls?: ToolCallSummary[]
  // RFC-015 chunk fields
  tool_call_id?: string
  status?: 'started' | 'completed'
  summary?: string
  output_summary?: string
  is_error?: boolean
  /// Human-readable progress text (RFC-015 v0.12).
  progress?: string
  /// Browser tab id (when the upstream tool is tab-aware, e.g. browser).
  /// Absent on legacy oxi-agent versions; the frontend treats absence
  /// as "no badge".
  tab_id?: string
  action?: 'recall' | 'store'
  query?: string
  count?: number
  source?: string
  input_tokens?: number
  output_tokens?: number
  /// Semantic context from the tool call (oxi-agent 0.29+ BrowseProgress).
  /// Carries structured info about what a browsing tool is doing.
  /// UI consumers that understand a context kind render it richly;
  /// older consumers simply ignore the field.
  context?: ToolCallContext
  // ── Interview chunk fields (chat UI redesign) ──
  questions?: InterviewQuestion[]
  round?: number
  ambiguity?: number
  // ── Tool approval (RFC-017) ──
  id?: string
  reason?: string
}

// ── Browser observability (RFC-015 Phase G, oxi-agent 0.29.1+) ─────────

/** Reason for visiting a page. Mirrors oxi-agent's `VisitReason` enum. */
export type VisitReason =
  | 'direct_navigation'
  | { search_result: { position: number } }
  | { link_followed: { from_url: string } }

/** Screenshot metadata. Mirrors oxi-agent's `ScreenshotMeta` struct. */
export interface ScreenshotMeta {
  /** PNG payload size in bytes. */
  bytes: number
  /** Viewport width. */
  width: number
  /** Capture duration in milliseconds. */
  duration_ms: number
}

/** Semantic context for a browsing tool execution event. */
export type ToolCallContext =
  | { kind: 'web_search'; query: string; engine?: string }
  | {
      kind: 'page_visit'
      url: string
      reason?: VisitReason
      page_title?: string
      page_status?: number
      page_bytes?: number
      page_duration_ms?: number
      /** Navigation error (from BrowseProgress::NavigationFailed, oxi-agent 0.29.1+). */
      navigation_error?: string
      /** Screenshot metadata (from BrowseProgress::ScreenshotCaptured, oxi-agent 0.29.1+). */
      screenshot?: ScreenshotMeta
    }
  | {
      kind: 'data_extraction'
      target: string
      url?: string
      result_count?: number
      page_status?: number
      page_duration_ms?: number
    }
  | { kind: 'session_action'; action: string; url?: string }
  | { kind: 'script_step'; current: number; total: number; step: string }

// Event (SSE)
export interface OxiosEvent {
  id?: string
  type: string
  agent_id?: string
  session_id?: string
  timestamp?: string
  data?: Record<string, unknown>
  // SSE events may also carry ad-hoc fields
  [key: string]: unknown
}

// Approval
export interface Approval {
  id: string
  subject: string
  action: string
  resource: string
  reason: string
  created_at: string
  status: string
}

// Cron Job
export interface CronJob {
  id: string
  name: string
  schedule: string
  command?: string
  enabled: boolean
  last_run?: string
  next_run?: string
}

// Budget — moved to types/budget.ts

// Resource
export interface ResourceSnapshot {
  timestamp: string
  cpu_percent: number
  memory_percent: number
  disk_percent: number
}

// Audit
export interface AuditEntry {
  id?: string
  agent_id?: string
  agent_name?: string
  action: string
  resource?: string
  allowed?: boolean
  reason?: string | null
  timestamp: string
  details?: Record<string, unknown>
  hash?: string
}

// Git
export interface GitCommit {
  hash: string
  message: string
  author: string
  timestamp: string
}

// Persona
export interface Persona {
  id: string
  name: string
  role?: string
  description?: string
  enabled: boolean
  personality_traits?: string[]
}

// Workspace — matches backend TreeEntry from /api/workspace/tree
export interface TreeEntry {
  name: string
  is_dir: boolean
  size: number
}

// Paginated response
export interface PaginatedResponse<T> {
  items: T[]
  total: number
  page: number
  limit: number
}

// Status — matches backend StatusResponse
export interface SystemStatus {
  service: string
  status: string
  version: string
  channels: string[]
  uptime: string // formatted "1h 30m 5s"
  components?: {
    state_store?: { healthy: boolean; detail?: string }
    event_bus?: { healthy: boolean; detail?: string }
    memory?: { enabled: boolean; index_size: number; total_entries: number }
    agents?: {
      active_count: number
      total_forked: number
      total_completed: number
      total_failed: number
    }
    spaces_active?: number
    projects_active?: number
  }
}

// ClawHub / Marketplace (RFC-010)

export interface ClawHubSearchResult {
  score: number
  slug: string
  displayName: string
  summary?: string
  version?: string
  updatedAt?: number
}

export interface ClawHubSkillDetail {
  skill: {
    slug: string
    displayName: string
    summary?: string
    tags?: Record<string, string>
    createdAt: number
    updatedAt: number
  } | null
  latestVersion?: {
    version: string
    createdAt: number
    changelog?: string
  }
  metadata?: {
    os?: string[]
    systems?: string[]
  }
  owner?: {
    handle?: string
    displayName?: string
    image?: string
  }
}

// Skills.sh (Vercel Labs ecosystem)

export interface SkillsShSkill {
  id: string
  slug: string
  name: string
  source: string
  installs: number
  sourceType: string
  installUrl?: string
  url: string
  isDuplicate?: boolean
}

export interface SkillsShSearchResponse {
  data: SkillsShSkill[]
  query: string
  searchType: string
  count: number
  durationMs?: number
}

export interface SkillsShListResponse {
  data: SkillsShSkill[]
  pagination: {
    page: number
    perPage: number
    total: number
    hasMore: boolean
  }
}

export interface SkillsShSkillDetail {
  id: string
  source: string
  slug: string
  installs: number
  hash?: string
  files?: Array<{
    path: string
    contents: string
  }>
}

export interface SkillsShAuditEntry {
  provider: string
  slug: string
  status: string
  summary: string
  auditedAt: string
  riskLevel?: string
}

export interface SkillsShAuditResponse {
  id: string
  source: string
  slug: string
  audits: SkillsShAuditEntry[]
}

// System Update
export interface UpdateCheckResponse {
  current_version: string
  latest_version: string
  update_available: boolean
  tag_name: string
  html_url: string
  release_notes: string
  published_at: string
  assets: { name: string; size: number; download_url: string }[]
}

export interface UpdateRunResponse {
  success: boolean
  updated_to: string
  binary_updated: boolean
  web_updated: boolean
  message: string
}

export interface ChangelogResponse {
  tag_name: string
  version: string
  published_at: string
  body: string
  html_url: string
}

// System Tools
export interface DoctorCheck {
  name: string
  status: 'pass' | 'warn' | 'fail'
  message: string
}

export interface DoctorResponse {
  checks: number
  issues: number
  results: DoctorCheck[]
  action_items: string[]
}

export interface AuditVerifyResponse {
  valid: boolean
  entries_checked: number
  message: string
}

export interface BackupResponse {
  success: boolean
  path: string
  size_bytes: number
  message: string
}

export interface LogResponse {
  lines: string[]
  total: number
}

export * from './calendar'
