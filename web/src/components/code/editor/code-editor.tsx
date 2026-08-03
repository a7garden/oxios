import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands'
import {
  bracketMatching,
  defaultHighlightStyle,
  foldGutter,
  foldKeymap,
  indentOnInput,
  syntaxHighlighting,
} from '@codemirror/language'
import { EditorState } from '@codemirror/state'
import { oneDark } from '@codemirror/theme-one-dark'
import {
  crosshairCursor,
  drawSelection,
  EditorView,
  highlightActiveLine,
  highlightActiveLineGutter,
  highlightSpecialChars,
  keymap,
  lineNumbers,
  rectangularSelection,
} from '@codemirror/view'
import { FileText, Sparkles } from 'lucide-react'
import { useCallback, useEffect, useRef } from 'react'
import { toast } from 'sonner'

import { codeApi } from '@/lib/code-api'
import { getLanguageExtension } from '@/lib/cm6-language'
import { useCodeSessionStore } from '@/stores/code/code-session'
import type { EditorTab } from '@/types/code'

import { EditorTabs } from './editor-tabs'

/**
 * Center editor panel.
 *
 * Renders the open tabs above an active CodeMirror 6 view, or an empty-state
 * hero when nothing is open. The CM6 view is recreated whenever the active
 * tab changes (so language + doc stay in sync); per-keystroke edits flow
 * back into the store via `updateTab` so persistence, dirty flags, and
 * cross-component reads all see the latest content.
 */
export function CodeEditor() {
  const tabs = useCodeSessionStore((s) => s.tabs)
  const activeTabId = useCodeSessionStore((s) => s.activeTabId)
  const session = useCodeSessionStore((s) => s.session)
  const updateTab = useCodeSessionStore((s) => s.updateTab)

  const activeTab = tabs.find((t) => t.id === activeTabId) ?? null

  if (!activeTab) {
    return (
      <div className="flex h-full w-full flex-col overflow-hidden">
        <EditorTabs />
        <EmptyState projectName={session?.title ?? 'No project'} />
      </div>
    )
  }

  return (
    <div className="flex h-full w-full flex-col overflow-hidden">
      <EditorTabs />
      <ActiveEditor key={activeTab.id} tab={activeTab} updateTab={updateTab} />
    </div>
  )
}

function EmptyState({ projectName }: { projectName: string }) {
  return (
    <div className="flex flex-1 items-center justify-center bg-surface">
      <div className="flex max-w-sm flex-col items-center gap-3 text-center">
        <div className="flex h-12 w-12 items-center justify-center rounded-lg bg-surface-sunken text-muted-foreground">
          <Sparkles className="h-6 w-6" />
        </div>
        <div className="flex flex-col gap-1">
          <p className="text-sm font-medium text-foreground">{projectName}</p>
          <p className="text-xs text-muted-foreground">
            Open a file from the explorer to start editing.
          </p>
        </div>
      </div>
    </div>
  )
}

interface ActiveEditorProps {
  tab: EditorTab
  updateTab: (id: string, updates: Partial<EditorTab>) => void
}

/**
 * Renders the CM6 view for a single tab. The component is keyed by tab.id in
 * the parent so it remounts (and rebuilds the editor with the correct language)
 * whenever the user switches tabs.
 */
function ActiveEditor({ tab, updateTab }: ActiveEditorProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const viewRef = useRef<EditorView | null>(null)

  // Save current doc back to the store as dirty.
  const handleChange = useCallback(
    (content: string) => {
      updateTab(tab.id, { content, isDirty: true })
    },
    [tab.id, updateTab],
  )

  // Persist current doc to disk and clear the dirty flag.
  const handleSave = useCallback(async () => {
    if (!viewRef.current) return
    const content = viewRef.current.state.doc.toString()
    try {
      await codeApi.writeFile(tab.path, content)
      updateTab(tab.id, { isDirty: false })
      toast.success(`Saved ${tab.name}`, { duration: 1500 })
    } catch (err) {
      const message = err instanceof Error ? err.message : 'Unknown error'
      toast.error(`Failed to save ${tab.name}: ${message}`)
    }
  }, [tab.id, tab.name, tab.path, updateTab])

  // Build the CM6 view exactly once per tab mount. The parent keys this
  // component by `tab.id`, so remounts happen only when the user switches
  // tabs. Subsequent keystrokes flow through `updateListener` → store, never
  // through this effect, so the cursor and undo stack survive typing.
  useEffect(() => {
    const container = containerRef.current
    if (!container) return

    const langExt = getLanguageExtension(tab.path)

    const saveKeymap = keymap.of([
      {
        key: 'Mod-s',
        run: () => {
          // Fire and forget — the keymap handler must return sync.
          void handleSave()
          return true
        },
        preventDefault: true,
      },
    ])

    const updateListener = EditorView.updateListener.of((update) => {
      if (update.docChanged) {
        handleChange(update.state.doc.toString())
      }
    })

    const extensions = [
      lineNumbers(),
      highlightActiveLineGutter(),
      highlightSpecialChars(),
      drawSelection(),
      rectangularSelection(),
      crosshairCursor(),
      highlightActiveLine(),
      history(),
      foldGutter(),
      indentOnInput(),
      bracketMatching(),
      syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
      oneDark,
      keymap.of([...defaultKeymap, ...historyKeymap, ...foldKeymap, indentWithTab]),
      saveKeymap,
      updateListener,
      EditorView.theme({
        '&': { height: '100%' },
        '.cm-scroller': { overflow: 'auto', fontFamily: 'var(--font-mono)' },
        '.cm-content': { padding: '12px 0' },
      }),
    ]

    if (langExt) {
      extensions.push(langExt)
    }

    const state = EditorState.create({
      doc: tab.content,
      extensions,
    })

    const view = new EditorView({
      state,
      parent: container,
    })

    viewRef.current = view

    return () => {
      view.destroy()
      viewRef.current = null
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tab.id, tab.path])

  return (
    <div className="relative flex-1 overflow-hidden bg-surface">
      {/* Path breadcrumb floating above the editor */}
      <div className="pointer-events-none absolute left-3 top-2 z-10 flex items-center gap-1.5 text-[11px] text-muted-foreground">
        <FileText className="h-3 w-3" />
        <span className="truncate font-mono">{tab.path}</span>
      </div>
      <div ref={containerRef} className="h-full w-full overflow-hidden pt-7" />
    </div>
  )
}