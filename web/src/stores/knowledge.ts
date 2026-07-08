import { create } from 'zustand'
import { persist } from 'zustand/middleware'

interface KnowledgeState {
  // View mode
  mode: 'home' | 'editor' | 'chat'

  // Current file
  currentFilePath: string | null

  // Navigation history
  history: string[]
  historyIndex: number

  // Layout
  infoPanelOpen: boolean

  // Split editor
  splitEditorOpen: boolean
  splitFilePath: string | null

  // Actions
  openFile: (path: string) => void
  openChat: () => void
  openHome: () => void
  goBack: () => string | null | undefined
  goForward: () => string | null | undefined
  toggleInfoPanel: () => void
  openSplit: (path: string) => void
  closeSplit: () => void
}

export const useKnowledgeStore = create<KnowledgeState>()(
  persist(
    (set, get) => ({
      mode: 'home',
      currentFilePath: null,
      history: [],
      historyIndex: -1,
      infoPanelOpen: false,
      splitEditorOpen: false,
      splitFilePath: null,

      openFile: (path) => {
        const { history, historyIndex } = get()
        // Trim forward history
        const newHistory = [...history.slice(0, historyIndex + 1), path]
        set({
          mode: 'editor',
          currentFilePath: path,
          history: newHistory,
          historyIndex: newHistory.length - 1,
        })
      },

      openChat: () => {
        set({ mode: 'chat' })
      },
      openHome: () => {
        set({ mode: 'home' })
      },

      goBack: () => {
        const { history, historyIndex } = get()
        if (historyIndex <= 0) return null
        const newIndex = historyIndex - 1
        const path = history[newIndex]
        set({ historyIndex: newIndex, currentFilePath: path, mode: 'editor' })
        return path
      },

      goForward: () => {
        const { history, historyIndex } = get()
        if (historyIndex >= history.length - 1) return null
        const newIndex = historyIndex + 1
        const path = history[newIndex]
        set({ historyIndex: newIndex, currentFilePath: path, mode: 'editor' })
        return path
      },

      toggleInfoPanel: () => {
        const infoPanelOpen = !get().infoPanelOpen
        // Opening the info panel closes the split editor so the layout
        // never exceeds three panes (tree + editor + one side panel).
        set(
          infoPanelOpen
            ? { infoPanelOpen: true, splitEditorOpen: false }
            : { infoPanelOpen: false },
        )
      },

      openSplit: (path) => {
        set({ splitEditorOpen: true, splitFilePath: path, infoPanelOpen: false })
      },

      closeSplit: () => {
        set({ splitEditorOpen: false, splitFilePath: null })
      },
    }),
    {
      name: 'oxios-knowledge',
      partialize: () => ({}),
    },
  ),
)
