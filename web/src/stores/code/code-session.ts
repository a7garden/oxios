import { create } from 'zustand'
import { persist } from 'zustand/middleware'
import type {
  CodeSession,
  FileChange,
  Checkpoint,
  TodoItem,
  CodeMessage,
  EditorTab,
} from '@/types/code'

interface CodeSessionStore {
  session: CodeSession | null
  pendingChanges: FileChange[]
  checkpoints: Checkpoint[]
  todos: TodoItem[]
  messages: CodeMessage[]
  gitBranch: string | null
  tabs: EditorTab[]
  activeTabId: string | null
  terminalIds: string[]
  isAgentRunning: boolean
  agentPhase: string | null

  setSession: (s: CodeSession | null) => void
  setSessionState: (data: {
    pending_changes?: number
    checkpoints?: Checkpoint[]
    git_branch?: string | null
  }) => void
  addMessage: (msg: CodeMessage) => void
  updateMessage: (id: string, updates: Partial<CodeMessage>) => void
  setPendingChanges: (c: FileChange[]) => void
  setCheckpoints: (c: Checkpoint[]) => void
  setTodos: (t: TodoItem[]) => void
  addTab: (tab: EditorTab) => void
  closeTab: (id: string) => void
  setActiveTab: (id: string) => void
  updateTab: (id: string, updates: Partial<EditorTab>) => void
  addTerminal: (id: string) => void
  removeTerminal: (id: string) => void
  setAgentRunning: (r: boolean) => void
  setAgentPhase: (p: string | null) => void
  reset: () => void
}

export const useCodeSessionStore = create<CodeSessionStore>((set) => ({
  session: null,
  pendingChanges: [],
  checkpoints: [],
  todos: [],
  messages: [],
  gitBranch: null,
  tabs: [],
  activeTabId: null,
  terminalIds: [],
  isAgentRunning: false,
  agentPhase: null,

  setSession: (session) => set({ session }),
  setSessionState: (data) =>
    set((s) => ({
      checkpoints: data.checkpoints ?? s.checkpoints,
      gitBranch: data.git_branch ?? s.gitBranch,
    })),
  addMessage: (msg) => set((s) => ({ messages: [...s.messages, msg] })),
  updateMessage: (id, updates) =>
    set((s) => ({
      messages: s.messages.map((m) => (m.id === id ? { ...m, ...updates } : m)),
    })),
  setPendingChanges: (changes) => set({ pendingChanges: changes }),
  setCheckpoints: (cps) => set({ checkpoints: cps }),
  setTodos: (todos) => set({ todos }),
  addTab: (tab) =>
    set((s) => {
      const existing = s.tabs.find((t) => t.path === tab.path)
      if (existing) return { tabs: s.tabs, activeTabId: existing.id }
      return { tabs: [...s.tabs, tab], activeTabId: tab.id }
    }),
  closeTab: (id) =>
    set((s) => {
      const idx = s.tabs.findIndex((t) => t.id === id)
      const tabs = s.tabs.filter((t) => t.id !== id)
      let activeTabId = s.activeTabId
      if (s.activeTabId === id) {
        activeTabId = tabs[Math.max(0, idx - 1)]?.id ?? null
      }
      return { tabs, activeTabId }
    }),
  setActiveTab: (id) => set({ activeTabId: id }),
  updateTab: (id, updates) =>
    set((s) => ({
      tabs: s.tabs.map((t) => (t.id === id ? { ...t, ...updates } : t)),
    })),
  addTerminal: (id) => set((s) => ({ terminalIds: [...s.terminalIds, id] })),
  removeTerminal: (id) =>
    set((s) => ({ terminalIds: s.terminalIds.filter((t) => t !== id) })),
  setAgentRunning: (running) => set({ isAgentRunning: running }),
  setAgentPhase: (phase) => set({ agentPhase: phase }),
  reset: () =>
    set({
      session: null,
      pendingChanges: [],
      checkpoints: [],
      todos: [],
      messages: [],
      gitBranch: null,
      tabs: [],
      activeTabId: null,
      terminalIds: [],
      isAgentRunning: false,
      agentPhase: null,
    }),
}))

interface CodeLayoutStore {
  explorerWidth: number
  agentWidth: number
  terminalHeight: number
  showExplorer: boolean
  showAgent: boolean
  showTerminal: boolean
  showCanvas: boolean
  setExplorerWidth: (w: number) => void
  setAgentWidth: (w: number) => void
  setTerminalHeight: (h: number) => void
  toggleExplorer: () => void
  toggleAgent: () => void
  toggleTerminal: () => void
  toggleCanvas: () => void
}

export const useCodeLayoutStore = create<CodeLayoutStore>()(
  persist(
    (set) => ({
      explorerWidth: 18,
      agentWidth: 28,
      terminalHeight: 25,
      showExplorer: true,
      showAgent: true,
      showTerminal: false,
      showCanvas: false,
      setExplorerWidth: (w) => set({ explorerWidth: w }),
      setAgentWidth: (w) => set({ agentWidth: w }),
      setTerminalHeight: (h) => set({ terminalHeight: h }),
      toggleExplorer: () => set((s) => ({ showExplorer: !s.showExplorer })),
      toggleAgent: () => set((s) => ({ showAgent: !s.showAgent })),
      toggleTerminal: () => set((s) => ({ showTerminal: !s.showTerminal })),
      toggleCanvas: () => set((s) => ({ showCanvas: !s.showCanvas })),
    }),
    { name: 'oxios-code-layout' },
  ),
)
