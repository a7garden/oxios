// Tree
export interface KnowledgeTreeEntry {
  name: string
  is_dir: boolean
  size: number
  /** RFC-022: note quality from frontmatter. null/undefined = user-written. */
  oxios_quality?: 'raw' | 'curated' | 'refined' | null
}

// Search
export interface KnowledgeSearchHit {
  path: string
  name: string
  snippet: string
  backlink_count: number
  name_similarity: number
}

export interface KnowledgeSearchResult {
  results: KnowledgeSearchHit[]
  count: number
  query: string
}

// Backlinks
export interface KnowledgeBacklink {
  source_path: string
  link_text: string
  context: string
}

// Graph
export interface KnowledgeGraphNode {
  id: string
  label: string
  group: string
}

export interface KnowledgeGraphEdge {
  source: string
  target: string
  label: string
}

export interface KnowledgeGraph {
  nodes: KnowledgeGraphNode[]
  edges: KnowledgeGraphEdge[]
}

// Copilot
export interface KnowledgeCopilotResponse {
  content: string
  referenced_notes: string[]
}

// Checklist
export interface ChecklistItemsResponse {
  items: string[]
  incomplete: string[]
}

// Journal
export interface JournalTodayResponse {
  path: string
}

// Stats
export interface TodayReport {
  // flexible - serialized from oxios_markdown::stats::TodayReport
  [key: string]: unknown
}

// Schedule config for knowledge module
export interface ScheduleConfig {
  name: string
  cron?: string
  command?: string
  enabled?: boolean
}

// Config
export interface KnowledgeConfig {
  language?: string
  timezone?: string
  move_to_commands?: string[]
  pomodoro_duration_in_minutes?: number
  schedules?: ScheduleConfig[]
  quick_commands?: string[]
  two_emojis_enabled?: boolean
  mode?: string
  quick_habits_enabled?: boolean
  channels?: number[]
}

// Convert
export interface ConvertHtmlResponse {
  html: string
}

// Emoji
export interface EmojiResponse {
  emoji: string
}

// Worker
export interface NightlyReport {
  [key: string]: unknown
}

// Habits - flexible structure from oxios_markdown
export interface HabitsData {
  [key: string]: unknown
}

// Git version history
export interface KnowledgeHistoryEntry {
  hash: string
  short_hash: string
  message: string
  timestamp: string
  author: string
}

export interface KnowledgeHistoryResponse {
  history: KnowledgeHistoryEntry[]
  count: number
}

// File diff
export interface FileDiffResponse {
  diff: string
  has_changes: boolean
}
