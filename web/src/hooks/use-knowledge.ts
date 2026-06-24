import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api } from '@/lib/api-client'
import type {
  ChecklistItemsResponse,
  ConvertHtmlResponse,
  EmojiResponse,
  HabitsData,
  JournalTodayResponse,
  KnowledgeBacklink,
  KnowledgeConfig,
  KnowledgeCopilotResponse,
  KnowledgeGraph,
  KnowledgeHistoryResponse,
  KnowledgeSearchResult,
  KnowledgeTreeEntry,
  NightlyReport,
  TodayReport,
} from '@/types/knowledge'

// F7: encode a knowledge-base file path for safe interpolation into a URL.
// Each path segment is encoded individually so '/' separators are preserved
// while characters like '?', '#', spaces, and non-ASCII bytes are escaped.
function encodeFilePath(path: string): string {
  return path
    .split('/')
    .map((seg) => encodeURIComponent(seg))
    .join('/')
}
// ── File I/O ──────────────────────────────────────────────────

export function useKnowledgeTree(dir?: string) {
  return useQuery({
    queryKey: ['knowledge', 'tree', dir ?? ''],
    queryFn: () => api.get<KnowledgeTreeEntry[]>('/api/knowledge/tree', dir ? { dir } : undefined),
  })
}

export function useKnowledgeFile(path: string | null) {
  return useQuery({
    queryKey: ['knowledge', 'file', path],
    queryFn: () => api.get<string>(`/api/knowledge/file/${encodeFilePath(path!)}`),
    enabled: !!path,
    staleTime: 0,
  })
}

export function useWriteFile() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ path, content }: { path: string; content: string }) =>
      api.put(`/api/knowledge/file/${encodeFilePath(path)}`, content, true), // raw markdown, not JSON
    onSuccess: (_, { path }) => {
      qc.invalidateQueries({ queryKey: ['knowledge', 'tree'] })
      qc.invalidateQueries({ queryKey: ['knowledge', 'file', path] })
      qc.invalidateQueries({ queryKey: ['knowledge', 'backlinks'] })
    },
  })
}

export function useDeleteFile() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (path: string) => api.delete(`/api/knowledge/file/${encodeFilePath(path)}`),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['knowledge', 'tree'] })
    },
  })
}

// ── Search ────────────────────────────────────────────────────

export function useKnowledgeSearch() {
  return useMutation({
    mutationFn: ({ query, limit = 20 }: { query: string; limit?: number }) =>
      api.post<KnowledgeSearchResult>('/api/knowledge/search', { query, limit }),
  })
}

// ── Backlinks & Graph ─────────────────────────────────────────

export function useKnowledgeBacklinks(path: string | null) {
  return useQuery({
    queryKey: ['knowledge', 'backlinks', path],
    queryFn: () =>
      api.get<KnowledgeBacklink[]>('/api/knowledge/backlinks', path ? { path } : undefined),
    enabled: !!path,
  })
}

export function useKnowledgeGraph() {
  return useQuery({
    queryKey: ['knowledge', 'graph'],
    queryFn: () => api.get<KnowledgeGraph>('/api/knowledge/graph'),
  })
}

// ── Copilot ───────────────────────────────────────────────────

export function useKnowledgeCopilot() {
  return useMutation({
    mutationFn: ({ question, contextPath }: { question: string; contextPath?: string }) =>
      api.post<KnowledgeCopilotResponse>('/api/knowledge/copilot', {
        question,
        context_path: contextPath,
      }),
  })
}

// ── Chat ──────────────────────────────────────────────────────

export function useChatMessages() {
  return useQuery({
    queryKey: ['knowledge', 'chat', 'messages'],
    queryFn: () => api.get<string[]>('/api/knowledge/chat/messages'),
  })
}

export function useChatAppend() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (message: string) => api.post('/api/knowledge/chat/append', { message }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['knowledge', 'chat', 'messages'] })
    },
  })
}

export function useChatDelete() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (msgHash: string) => api.post('/api/knowledge/chat/delete', { msg_hash: msgHash }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['knowledge', 'chat', 'messages'] })
    },
  })
}

export function useChatMove() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ msgHash, targetPath }: { msgHash: string; targetPath: string }) =>
      api.post('/api/knowledge/chat/move', { msg_hash: msgHash, target_path: targetPath }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['knowledge', 'chat', 'messages'] })
      qc.invalidateQueries({ queryKey: ['knowledge', 'tree'] })
    },
  })
}

// ── Checklist ─────────────────────────────────────────────────

export function useChecklistItems(path: string | null) {
  return useQuery({
    queryKey: ['knowledge', 'checklist', path],
    queryFn: () => api.post<ChecklistItemsResponse>('/api/knowledge/checklist/items', { path }),
    enabled: !!path,
  })
}

export function useChecklistAdd() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({
      path,
      item,
      checked = false,
    }: {
      path: string
      item: string
      checked?: boolean
    }) => api.post('/api/knowledge/checklist/add', { path, item, checked }),
    onSuccess: (_, { path }) => {
      qc.invalidateQueries({ queryKey: ['knowledge', 'checklist', path] })
      qc.invalidateQueries({ queryKey: ['knowledge', 'tree'] })
    },
  })
}

export function useChecklistComplete() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ path, itemHash }: { path: string; itemHash: string }) =>
      api.post('/api/knowledge/checklist/complete', { path, item_hash: itemHash }),
    onSuccess: (_, { path }) => {
      qc.invalidateQueries({ queryKey: ['knowledge', 'checklist', path] })
    },
  })
}

export function useChecklistRemove() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ path, itemOrHash }: { path: string; itemOrHash: string }) =>
      api.post('/api/knowledge/checklist/remove', { path, item_or_hash: itemOrHash }),
    onSuccess: (_, { path }) => {
      qc.invalidateQueries({ queryKey: ['knowledge', 'checklist', path] })
    },
  })
}

// ── Journal ───────────────────────────────────────────────────

export function useJournalToday() {
  return useQuery({
    queryKey: ['knowledge', 'journal', 'today'],
    queryFn: () => api.get<JournalTodayResponse>('/api/knowledge/journal/today'),
  })
}

export function useJournalAdd() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (record: string) => api.post('/api/knowledge/journal/add', { record }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['knowledge', 'tree'] })
    },
  })
}

export function useJournalAddEmoji() {
  return useMutation({
    mutationFn: (emoji: string) => api.post('/api/knowledge/journal/emoji', { emoji }),
  })
}

// ── Habits ────────────────────────────────────────────────────

export function useKnowledgeHabits(year?: number) {
  return useQuery({
    queryKey: ['knowledge', 'habits', year],
    queryFn: () =>
      api.get<HabitsData>('/api/knowledge/habits', year ? { year: String(year) } : undefined),
  })
}

export function useKnowledgeHabitsLastWeek() {
  return useQuery({
    queryKey: ['knowledge', 'habits', 'last-week'],
    queryFn: () => api.get<HabitsData>('/api/knowledge/habits/last-week'),
  })
}

// ── Stats ─────────────────────────────────────────────────────

export function useKnowledgeStatsToday() {
  return useQuery({
    queryKey: ['knowledge', 'stats', 'today'],
    queryFn: () => api.get<TodayReport>('/api/knowledge/stats/today'),
  })
}

export function useKnowledgeDoneToday() {
  return useQuery({
    queryKey: ['knowledge', 'stats', 'done-today'],
    queryFn: () => api.get<{ items: unknown[]; count: number }>('/api/knowledge/stats/done-today'),
  })
}

// ── Config ────────────────────────────────────────────────────

export function useKnowledgeConfig() {
  return useQuery({
    queryKey: ['knowledge', 'config'],
    queryFn: () => api.get<KnowledgeConfig>('/api/knowledge/config'),
  })
}

export function useKnowledgeConfigUpdate() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (config: Partial<KnowledgeConfig>) => api.put('/api/knowledge/config', config),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['knowledge', 'config'] })
    },
  })
}

// ── Worker ────────────────────────────────────────────────────

export function useNightlyCleanup() {
  return useMutation({
    mutationFn: () => api.post<NightlyReport>('/api/knowledge/worker/nightly'),
  })
}

export function useScheduledTasks() {
  return useMutation({
    mutationFn: () =>
      api.post<{ moved: string[]; count: number }>('/api/knowledge/worker/scheduled'),
  })
}

// ── Convert ───────────────────────────────────────────────────

export function useConvertHtml() {
  return useMutation({
    mutationFn: (md: string) =>
      api.post<ConvertHtmlResponse>('/api/knowledge/convert/html', { md }),
  })
}

// ── Emoji ─────────────────────────────────────────────────────

export function useAutoEmoji(text: string) {
  return useQuery({
    queryKey: ['knowledge', 'emoji', text],
    queryFn: () => api.get<EmojiResponse>('/api/knowledge/emoji', { text }),
    enabled: text.length > 0,
  })
}

// ── Git Version History ────────────────────────────────────────

export function useKnowledgeFileHistory(path: string | null) {
  return useQuery({
    queryKey: ['knowledge', 'history', path],
    queryFn: () =>
      api.get<KnowledgeHistoryResponse>(`/api/knowledge/file/${encodeFilePath(path!)}/history`),
    enabled: !!path,
  })
}

export function useKnowledgeFileRestore() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: ({ path, hash }: { path: string; hash: string }) =>
      api.post(`/api/knowledge/file/${encodeFilePath(path)}/restore`, { hash }),
    onSuccess: (_, { path }) => {
      qc.invalidateQueries({ queryKey: ['knowledge', 'file', path] })
      qc.invalidateQueries({ queryKey: ['knowledge', 'history', path] })
      qc.invalidateQueries({ queryKey: ['knowledge', 'backlinks'] })
    },
  })
}
