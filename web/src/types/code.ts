/** Code Workspace domain types. */

export interface CodeSession {
  id: string
  project_path: string
  model: string | null
  created_at: string
  title: string
}

export interface SessionResponse {
  session: CodeSession
  pending_changes: number
  checkpoints: Checkpoint[]
  git_branch: string | null
}

export interface DirEntry {
  name: string
  path: string
  is_dir: boolean
  is_file: boolean
  size: number | null
  modified: number | null
}

export interface FileContent {
  content: string
  language: string
  path: string
}

export type ChangeAction = 'create' | 'modify' | 'delete'

export interface FileChange {
  path: string
  action: ChangeAction
  original_content: string | null
  new_content: string | null
  diff: string
  timestamp: string
  accepted: boolean
  tool_call_id: string | null
}

export interface Checkpoint {
  id: string
  description: string
  timestamp: string
  files: string[]
}

export type TodoStatus = 'pending' | 'in_progress' | 'done'

export interface TodoItem {
  id: string
  text: string
  status: TodoStatus
}

export interface TerminalInfo {
  terminal_id: string
}

export interface FsSearchResult {
  file: string
  line: number
  text: string
}

/** WebSocket chunk types for the coding session stream. */
export interface CodeWsChunk {
  type: string
  [key: string]: unknown
}

export interface ToolCallInfo {
  tool: string
  args: Record<string, unknown>
  result_summary?: string
  exit_code?: number
}

export interface CodeMessage {
  id: string
  role: 'user' | 'assistant' | 'system'
  content: string
  timestamp: string
  tool_calls?: ToolCallInfo[]
  model?: string
}

export interface EditorTab {
  id: string
  path: string
  name: string
  language: string
  isDirty: boolean
  isPreview: boolean
  content: string
}
