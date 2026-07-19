// ── Extended chat types (ported from LobeHub @lobechat/types) ──
// Fields that Oxios ChatMessage doesn't have yet.

// ── Error (ported from @lobechat/types message/common/base.ts) ──

export type ChatErrorAttribution = 'user' | 'provider' | 'harness' | 'system'
export type ChatErrorSeverity = 'info' | 'warning' | 'error' | 'critical'

export interface ChatError {
  attribution?: ChatErrorAttribution
  body?: unknown
  category?: string  // 'auth' | 'quota' | 'capacity' | 'routing' | ...
  httpStatus?: number
  message?: string
  retryable?: boolean
  severity?: ChatErrorSeverity
  type: string  // ErrorType from model-runtime
}

// ── Reasoning (ported from @lobechat/types message/common/base.ts) ──

export interface ModelReasoning {
  content: string
  duration?: number  // milliseconds
  /** Whether this turn is still streaming its reasoning block. */
  thinking?: boolean
}

// ── Search grounding (ported from @lobechat/types search.ts) ──

export interface CitationItem {
  favicon?: string
  id?: string
  title?: string
  url: string
}

export interface ImageCitationItem {
  domain?: string
  imageUri?: string
  sourceUri?: string
  title?: string
}

export interface GroundingSearch {
  citations?: CitationItem[]
  imageResults?: ImageCitationItem[]
  imageSearchQueries?: string[]
  searchQueries?: string[]
}

// ── File chunks (RAG references) ──

export interface ChatFileChunk {
  id: string
  content: string
  filename?: string
  score?: number
}

// ── Image item ──

export interface ChatImageItem {
  alt?: string
  url: string
}

// ── File item ──

export interface ChatFileItem {
  id: string
  name: string
  size: number
  type: string
  url?: string
}

// ── Tool render types ──

export interface ToolRenderProps {
  toolName: string
  args: Record<string, unknown>
  result: unknown
  isRunning: boolean
  durationMs?: number
}

// ── Extended ChatMessage (additions to existing ChatMessage) ──
// These fields will be merged into the main ChatMessage type.
// Existing fields: id, role, content, model, timestamp,
//   toolName, toolArgs, toolResult, toolDurationMs,
//   metadata (phase, evaluation_passed, duration_ms, tool_calls, isError, errorKind),
//   activities, totalInputTokens, totalOutputTokens,
//   _interviewQuestions, _interviewRound

/** LobeHub-ported fields to add to ChatMessage. */
export interface ChatMessageExtensions {
  /** Thinking/reasoning block. Displayed before the answer prose. */
  reasoning?: ModelReasoning | null
  /** Web search grounding with citation cards. */
  search?: GroundingSearch | null
  /** RAG reference chunks from knowledge base. */
  chunksList?: ChatFileChunk[]
  /** Generated or attached images. */
  imageList?: ChatImageItem[]
  /** Attached files. */
  fileList?: ChatFileItem[]
  /** Rich error with classification. */
  error?: ChatError | null
  /** Whether this message is currently generating. */
  generating?: boolean
  /** Whether this message is in reasoning phase. */
  isReasoning?: boolean
  /** Whether tool calls are being generated. */
  isToolCallGenerating?: boolean
  /** Whether this message is collapsed (compressed context). */
  isCollapsed?: boolean
}

// ── Chat item props (ported from LobeHub ChatItem) ──

export interface ChatItemAvatar {
  name?: string
  avatar?: string  // URL or emoji
  color?: string
}

export interface ChatItemProps {
  id?: string
  avatar: ChatItemAvatar
  placement?: 'left' | 'right'
  loading?: boolean
  error?: ChatError | null
  time?: number  // unix ms
  showTitle?: boolean
  showAvatar?: boolean
  actions?: React.ReactNode
  messageExtra?: React.ReactNode
  children: React.ReactNode
  className?: string
}
