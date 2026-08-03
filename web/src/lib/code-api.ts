/** API helpers for Code Workspace endpoints. */
import { apiClient } from '@/lib/api-client'
import type {
  CodeSession,
  SessionResponse,
  DirEntry,
  FileContent,
  FileChange,
  Checkpoint,
  FsSearchResult,
  TerminalInfo,
} from '@/types/code'

export const codeApi = {
  // Sessions
  createSession: (projectPath: string, model?: string) =>
    apiClient<CodeSession>('/api/code/sessions', {
      method: 'POST',
      body: { project_path: projectPath, model: model ?? null },
    }),

  listSessions: () => apiClient<CodeSession[]>('/api/code/sessions'),

  getSession: (id: string) =>
    apiClient<SessionResponse>(`/api/code/sessions/${id}`),

  deleteSession: (id: string) =>
    apiClient<void>(`/api/code/sessions/${id}`, { method: 'DELETE' }),

  // Filesystem
  browse: (path: string) =>
    apiClient<DirEntry[]>('/api/code/fs/browse', { params: { path } }),

  readFile: (path: string) =>
    apiClient<FileContent>('/api/code/fs/read', { params: { path } }),

  writeFile: (path: string, content: string) =>
    apiClient<void>('/api/code/fs/write', {
      method: 'PUT',
      body: content,
      rawBody: true,
      params: { path },
    }),

  createFile: (path: string, isDir = false) =>
    apiClient<void>('/api/code/fs/create', {
      method: 'POST',
      body: { path, is_dir: isDir },
    }),

  deleteFile: (path: string) =>
    apiClient<void>('/api/code/fs/delete', { method: 'DELETE', params: { path } }),

  moveFile: (from: string, to: string) =>
    apiClient<void>('/api/code/fs/move', { method: 'POST', body: { from, to } }),

  // Search
  searchFiles: (path: string, q: string, limit?: number) =>
    apiClient<FsSearchResult[]>('/api/code/fs/search', {
      params: { path, q, ...(limit ? { limit } : {}) },
    }),

  listFiles: (path: string) =>
    apiClient<string[]>('/api/code/fs/list', { params: { path } }),

  // Changes
  listChanges: (sessionId: string) =>
    apiClient<FileChange[]>(`/api/code/sessions/${sessionId}/changes`),

  acceptAllChanges: (sessionId: string) =>
    apiClient<void>(`/api/code/sessions/${sessionId}/changes/accept-all`, {
      method: 'POST',
    }),

  rejectAllChanges: (sessionId: string) =>
    apiClient<void>(`/api/code/sessions/${sessionId}/changes/reject-all`, {
      method: 'POST',
    }),

  // Checkpoints
  createCheckpoint: (sessionId: string, description: string) =>
    apiClient<Checkpoint>(`/api/code/sessions/${sessionId}/checkpoint`, {
      method: 'POST',
      body: { description },
    }),

  listCheckpoints: (sessionId: string) =>
    apiClient<Checkpoint[]>(`/api/code/sessions/${sessionId}/checkpoints`),

  revertCheckpoint: (sessionId: string, cpId: string) =>
    apiClient<void>(`/api/code/sessions/${sessionId}/checkpoints/${cpId}/revert`, {
      method: 'POST',
    }),

  // Terminal
  createTerminal: (sessionId: string, shell?: string) =>
    apiClient<TerminalInfo>(`/api/code/sessions/${sessionId}/terminal`, {
      method: 'POST',
      body: { shell: shell ?? null },
    }),

  deleteTerminal: (tid: string) =>
    apiClient<void>(`/api/code/terminal/${tid}`, { method: 'DELETE' }),

  /** Build the WebSocket URL for a terminal. */
  terminalWsUrl: (tid: string): string => {
    const proto = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
    const base = import.meta.env.VITE_API_BASE || ''
    return `${proto}//${window.location.host}${base}/api/code/terminal/${tid}`
  },
}
