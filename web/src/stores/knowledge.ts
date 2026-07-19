import { create } from 'zustand'
import { persist } from 'zustand/middleware'

interface KnowledgeState {
  // View mode
  mode: 'home' | 'editor' | 'chat'

  // Current file
  currentFilePath: string | null
  /**
   * Monotonic counter bumped ONLY on explicit file switches (openFile /
   * goBack / goForward). Drives the editor's `key` so the editor fully
   * remounts when the user navigates to a different file. In-place
   * renames (renameCurrent) deliberately do NOT bump it — the editor
   * stays mounted across a rename so cursor + undo history survive, and
   * the content (kept warm in the React Query cache by useMoveFile's
   * from→to migration) replaces itself as a no-op.
   */
  editorSessionId: number

  // Navigation history
  history: string[]
  historyIndex: number

  // Layout
  infoPanelOpen: boolean

  // Split editor
  splitEditorOpen: boolean
  splitFilePath: string | null

  // Tree expansion (D2 — Phase 2)
  expandedPaths: string[]
  // Focused tree item (D6 — roving tabindex, Phase 5)
  focusedPath: string | null
  // Path of the most recently created file (Phase 5 §8.4: file-blink animation).
  // Cleared automatically after a short timeout to limit blast radius.
  recentlyCreatedPath: string | null
  // Actions
  openFile: (path: string) => void
  /**
   * Swap the currently open file's path in place (used after a rename —
   * either H1-driven or F2). Updates `currentFilePath` and rewrites the
   * current history entry so back/forward navigation stays consistent,
   * WITHOUT bumping `editorSessionId` (the editor must not remount).
   */
  renameCurrent: (newPath: string) => void
  openChat: () => void
  openHome: () => void
  goBack: () => string | null | undefined
  goForward: () => string | null | undefined
  toggleInfoPanel: () => void
  openSplit: (path: string) => void
  closeSplit: () => void

  // Tree actions
  toggleExpand: (path: string) => void
  expandPath: (path: string) => void // idempotent
  collapseAll: () => void
  expandToPath: (filePath: string) => void // expand all parent dirs
  setFocus: (path: string | null) => void
  // Phase 5 §8.4 — file-blink animation. Triggers the highlight on
  // the given path and clears it after ~1.5s.
  markFileCreated: (path: string) => void
}

/** Expand every parent directory of `filePath`. E.g. "brain/rust/x.md" → ["brain", "brain/rust"]. */
function expandToPathSegments(filePath: string): string[] {
  const out: string[] = []
  if (filePath.includes('/')) {
    const dirs = filePath.split('/').slice(0, -1)
    let acc = ''
    for (const dir of dirs) {
      acc = acc ? `${acc}/${dir}` : dir
      out.push(acc)
    }
  }
  return out
}

export const useKnowledgeStore = create<KnowledgeState>()(
  persist(
    (set, get) => ({
      mode: 'home',
      currentFilePath: null,
      editorSessionId: 0,
      history: [],
      historyIndex: -1,
      infoPanelOpen: false,
      splitEditorOpen: false,
      splitFilePath: null,
      expandedPaths: [],
      focusedPath: null,
      recentlyCreatedPath: null,

      openFile: (path) => {
        const { history, historyIndex } = get()
        // Trim forward history
        const newHistory = [...history.slice(0, historyIndex + 1), path]
        // Auto-expand ancestors so the active file is visible (§3.2 / Phase 2 step 7)
        const toExpand = expandToPathSegments(path)
        const expandedPaths = Array.from(new Set([...get().expandedPaths, ...toExpand]))
        set({
          mode: 'editor',
          currentFilePath: path,
          history: newHistory,
          historyIndex: newHistory.length - 1,
          expandedPaths,
          // New file → fresh editor session (remount, reset undo history).
          editorSessionId: get().editorSessionId + 1,
        })
      },
      renameCurrent: (newPath) => {
        const { currentFilePath, history, historyIndex } = get()
        if (!currentFilePath || newPath === currentFilePath) return
        // Rewrite the current history entry in place so back/forward
        // navigation stays consistent, WITHOUT bumping editorSessionId
        // (the editor stays mounted across a rename).
        const newHistory = history.slice()
        if (historyIndex >= 0 && historyIndex < newHistory.length) {
          newHistory[historyIndex] = newPath
        }
        set({ currentFilePath: newPath, history: newHistory })
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
        set({
          historyIndex: newIndex,
          currentFilePath: path,
          mode: 'editor',
          editorSessionId: get().editorSessionId + 1,
        })
        return path
      },

      goForward: () => {
        const { history, historyIndex } = get()
        if (historyIndex >= history.length - 1) return null
        const newIndex = historyIndex + 1
        const path = history[newIndex]
        set({
          historyIndex: newIndex,
          currentFilePath: path,
          mode: 'editor',
          editorSessionId: get().editorSessionId + 1,
        })
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

      // ── Tree actions ──
      toggleExpand: (path) => {
        const cur = get().expandedPaths
        set({
          expandedPaths: cur.includes(path) ? cur.filter((p) => p !== path) : [...cur, path],
        })
      },
      expandPath: (path) => {
        const cur = get().expandedPaths
        if (!cur.includes(path)) set({ expandedPaths: [...cur, path] })
      },
      collapseAll: () => set({ expandedPaths: [] }),
      expandToPath: (filePath) => {
        const toExpand = expandToPathSegments(filePath)
        if (toExpand.length === 0) return
        set({
          expandedPaths: Array.from(new Set([...get().expandedPaths, ...toExpand])),
        })
      },
      setFocus: (path) => set({ focusedPath: path }),

      markFileCreated: (path) => {
        set({ recentlyCreatedPath: path })
        if (typeof window !== 'undefined') {
          window.setTimeout(() => {
            const cur = get().recentlyCreatedPath
            // Only clear if still pointing at this file (avoid clobbering a
            // newer blink that started during the timeout window).
            if (cur === path) set({ recentlyCreatedPath: null })
          }, 1500)
        }
      },
    }),
    {
      name: 'oxios-knowledge',
      partialize: (s) => ({
        expandedPaths: s.expandedPaths,
        focusedPath: s.focusedPath,
      }),
    },
  ),
)
