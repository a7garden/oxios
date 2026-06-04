/**
 * Oxios knowledge-base markdown editor — CodeMirror 6 (Phase 1).
 *
 * Replaces HyperMD/CodeMirror 5 (deprecated, unmaintained since 2019)
 * with @uiw/react-codemirror + custom extensions.
 *
 * Phase 1 preserves the Obsidian/Logseq editing UX:
 *   - Plain markdown source view (not pure WYSIWYG)
 *   - Active-line-only markup visibility (default CM6)
 *   - All 5+ preserved features: auto-save, heading enforcement,
 *     ⌘B/⌘I/⌘Y, wiki/emoji autocomplete, Mod-S, dark/light, link click
 *
 * Phase 2 will add: image/code inline fold, wikilink click handler
 * Phase 3 will add: token hiding on inactive lines, mermaid widget, dark theme
 *
 * Why not Tiptap? See worktree exp/frontend-markdown-editor-poc/DECISION.md
 */
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import CodeMirror, {
  EditorView,
  type ReactCodeMirrorRef,
} from '@uiw/react-codemirror'
import { markdown, markdownLanguage } from '@codemirror/lang-markdown'
import { languages } from '@codemirror/language-data'
import { autocompletion, type Completion, type CompletionContext, type CompletionResult } from '@codemirror/autocomplete'
import { history, indentWithTab } from '@codemirror/commands'
import { bracketMatching, defaultHighlightStyle, syntaxHighlighting } from '@codemirror/language'
import { keymap } from '@codemirror/view'
import { oneDark } from '@codemirror/theme-one-dark'
import { mermaidExtension, mermaidDarkObserver } from '@/lib/mermaid-extension'
import { tokenHideExtension } from '@/lib/token-hide-extension'
import { wikilinkExtension } from '@/lib/wikilink-extension'
import { EditorSelection } from '@codemirror/state'
import { useKnowledgeTree } from '@/hooks/use-knowledge'
import { buildAutocompleteDict, type FileEntry } from '@/lib/autocomplete-link'
import { cn } from '@/lib/utils'
import { useKnowledgeStore } from '@/stores/knowledge'

interface MarkdownEditorProps {
  filePath: string
  initialContent: string
  onSave: (content: string) => void
  className?: string
}

// ─────────────────────────────────────────────────────────────────────────
// Custom keymap: ⌘B / Ctrl-B (bold), ⌘I / Ctrl-I (italic), ⌘Y (checklist),
// ⌘S / Ctrl-S (manual save via global 'knowledge:save' event)
// ─────────────────────────────────────────────────────────────────────────
const customKeymap = keymap.of([
  { key: 'Mod-b', run: wrapSelection('**', '**') },
  { key: 'Mod-i', run: wrapSelection('*', '*') },
  { key: 'Mod-y', run: insertCheckmark },
  { key: 'Mod-s', run: () => {
    document.dispatchEvent(new Event('knowledge:save'))
    return true
  } },
  indentWithTab,
])

function wrapSelection(before: string, after: string) {
  return (view: EditorView): boolean => {
    const { state } = view
    const changes = state.selection.ranges.map((range) => {
      const text = state.sliceDoc(range.from, range.to)
      return { from: range.from, to: range.to, insert: before + text + after }
    })
    if (changes.length === 0) return false
    view.dispatch({
      changes,
      selection: EditorSelection.create(
        changes.map((c) =>
          EditorSelection.range(c.from + before.length, c.to + before.length),
        ),
        1,
      ),
    })
    return true
  }
}

function insertCheckmark(view: EditorView): boolean {
  const { state } = view
  const line = state.doc.lineAt(state.selection.main.head)
  view.dispatch({
    changes: { from: line.from, insert: '- [x] ' },
  })
  return true
}

// ─────────────────────────────────────────────────────────────────────────
// Heading enforcement — keep first line as `# ` even after edit.
// Gated by a module-level flag set by the parent component so the
// enforcer does NOT fire while we're programmatically replacing the
// document content (which would cause an infinite loop: the enforcer
// dispatches a change → enforcer fires again → …).
// ─────────────────────────────────────────────────────────────────────────
let _headingEnforcerSuspended = false
const headingEnforcer = EditorView.updateListener.of((update) => {
  if (!update.docChanged) return
  if (_headingEnforcerSuspended) return
  const firstLine = update.state.doc.line(1)
  const text = firstLine.text
  if (!text.startsWith('# ')) {
    const content = text.replace(/^#*\s*/, '')
    update.view.dispatch({
      changes: { from: firstLine.from, to: firstLine.to, insert: `# ${content}` },
    })
  }
})

// ─────────────────────────────────────────────────────────────────────────
// Wiki link + emoji completion source
// ─────────────────────────────────────────────────────────────────────────
function makeCompletionSource(
  getEntries: () => FileEntry[],
  emojiDict: Record<string, string>,
) {
  return (ctx: CompletionContext): CompletionResult | null => {
    // Word range: alphanumeric + some markdown-safe chars
    const word = ctx.matchBefore(/[\p{L}\p{N}_\s:-]*/u)
    if (!word) return null
    if (word.from === word.to && !ctx.explicit) return null

    const before = ctx.state.sliceDoc(Math.max(0, word.from - 1), word.from)
    const fullText = ctx.state.sliceDoc(word.from, word.to)
    const lower = fullText.toLowerCase()

    const options: Completion[] = []

    // Wiki link: triggered by `[`
    if (before === '[') {
      const entries = getEntries()
      for (const e of entries) {
        if (!lower || e.key.toLowerCase().includes(lower)) {
          options.push({
            label: e.key,
            detail: e.filePath,
            apply: `${e.key}](${e.filePath.replace(/ /g, '%20')})`,
          })
          if (options.length >= 20) break
        }
      }
    }

    // Emoji: triggered by `:` at end
    if (before === ':' || lower.startsWith(':')) {
      const search = lower.replace(/^:/, '')
      for (const [key, icon] of Object.entries(emojiDict)) {
        if (!search || key.toLowerCase().includes(search)) {
          options.push({
            label: key,
            detail: icon,
            apply: `${icon} `,
          })
          if (options.length >= 20) break
        }
      }
    }

    if (options.length === 0) return null
    return {
      from: before === '[' || before === ':' ? word.from - 1 : word.from,
      to: word.to,
      options,
      validFor: /[\p{L}\p{N}_\s:-]*/u,
    }
  }
}

// Simple emoji dict (subset — extended in lib/emoji.ts)
const EMOJI_DICT: Record<string, string> = {
  heart: '❤️',
  smile: '😄',
  tada: '🎉',
  '+1': '👍',
  rocket: '🚀',
  fire: '🔥',
  check: '✅',
  x: '❌',
  warning: '⚠️',
  bulb: '💡',
}

// ─────────────────────────────────────────────────────────────────────────
// Link / wiki click handler — same semantics as HyperMD's hmdClick
// ─────────────────────────────────────────────────────────────────────────
const linkClickHandler = EditorView.domEventHandlers({
  click(event, _view) {
    const target = event.target as HTMLElement | null
    if (!target || target.tagName !== 'A') return false
    if (!(target instanceof HTMLAnchorElement)) return false
    const href = target.getAttribute('href') ?? ''
    if (!href) return false
    if (href.startsWith('http://') || href.startsWith('https://')) {
      window.open(href, '_blank', 'noopener')
      return true
    }
    if (href.startsWith('cmd:')) return true
    const path = href.endsWith('.md') ? href : `${href}.md`
    document.dispatchEvent(
      new CustomEvent('knowledge:open-file', { detail: { path } }),
    )
    return true
  },
})

// ─────────────────────────────────────────────────────────────────────────
// Editor base theme
// ─────────────────────────────────────────────────────────────────────────
const baseTheme = EditorView.theme({
  '&': {
    fontSize: '14px',
    height: '100%',
  },
  '.cm-scroller': {
    fontFamily:
      'ui-monospace, SFMono-Regular, "SF Mono", Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace',
  },
  '.cm-content': {
    padding: '12px 8px',
  },
  '.cm-gutters': {
    display: 'none',
  },
})

const darkTheme = EditorView.theme(
  {
    '&': { colorScheme: 'dark' },
  },
  { dark: true },
)

// ─────────────────────────────────────────────────────────────────────────
// Component
// ─────────────────────────────────────────────────────────────────────────
export function MarkdownEditor({
  filePath: _filePath,
  initialContent,
  onSave,
  className,
}: MarkdownEditorProps) {
  const ref = useRef<ReactCodeMirrorRef | null>(null)
  const viewRef = useRef<EditorView | null>(null)
  const [isDirty, setIsDirty] = useState(false)
  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined)
  const isSettingContent = useRef(false)
  const openFile = useKnowledgeStore((s) => s.openFile)
  const currentFilePath = useKnowledgeStore((s) => s.currentFilePath)
  const { data: treeEntries } = useKnowledgeTree()
  const [isDark, setIsDark] = useState(false)

  // Track dark mode via document class
  useEffect(() => {
    const obs = new MutationObserver(() => {
      setIsDark(document.documentElement.classList.contains('dark'))
    })
    obs.observe(document.documentElement, { attributes: true, attributeFilter: ['class'] })
    setIsDark(document.documentElement.classList.contains('dark'))
    return () => obs.disconnect()
  }, [])

  const onSaveRef = useRef(onSave)
  onSaveRef.current = onSave

  const autocompleteEntries = useCallback(() => {
    if (!treeEntries) return []
    return buildAutocompleteDict(treeEntries, undefined, currentFilePath ?? undefined)
  }, [treeEntries, currentFilePath])

  const completionSource = useMemo(
    () => makeCompletionSource(autocompleteEntries, EMOJI_DICT),
    [autocompleteEntries],
  )

  // Manual save handler (toolbar / ⌘S)
  useEffect(() => {
    const handler = () => {
      const view = viewRef.current
      if (!view) return
      if (saveTimerRef.current) clearTimeout(saveTimerRef.current)
      onSaveRef.current(view.state.doc.toString())
      setIsDirty(false)
    }
    document.addEventListener('knowledge:save', handler)
    return () => {
      // Cancel any pending debounce save on unmount so we don't
      // call onSave on a stale editor instance.
      if (saveTimerRef.current) {
        clearTimeout(saveTimerRef.current)
        saveTimerRef.current = undefined
      }
      document.removeEventListener('knowledge:save', handler)
    }
  }, [])

  // External open-file listener (from link click)
  useEffect(() => {
    const handler = (e: Event) => {
      const detail = (e as CustomEvent<{ path: string }>).detail
      if (detail?.path) openFile(detail.path)
    }
    document.addEventListener('knowledge:open-file', handler)
    return () => document.removeEventListener('knowledge:open-file', handler)
  }, [openFile])

  // Save on blur
  const handleBlur = useCallback(() => {
    const view = viewRef.current
    if (!view || !isDirty) return
    if (saveTimerRef.current) clearTimeout(saveTimerRef.current)
    onSaveRef.current(view.state.doc.toString())
    setIsDirty(false)
  }, [isDirty])

  // Update content when initialContent changes (file loaded from API)
  useEffect(() => {
    const view = viewRef.current
    if (!view) return
    const current = view.state.doc.toString()
    if (current === initialContent) return
    // Suspend the heading enforcer and onChange-driven autosave while
    // we programmatically replace the document. Combining the change
    // and the selection reset into a SINGLE dispatch avoids the
    // enforcer firing on the intermediate state (which has the
    // cursor at the end of the old content) and producing unwanted
    // headings or selection drift.
    isSettingContent.current = true
    _headingEnforcerSuspended = true
    view.dispatch({
      changes: { from: 0, to: current.length, insert: initialContent },
      selection: { anchor: 0 },
    })
    // Release on the next macrotask. `queueMicrotask` is too soon:
    // CM6 update listeners that schedule React state updates can
    // resolve on the next macrotask, and onChange can fire AFTER the
    // microtask. `setTimeout(0)` ensures the enforcer and onChange
    // gate stay in place until the editor has fully settled.
    const releaseTimer = setTimeout(() => {
      isSettingContent.current = false
      _headingEnforcerSuspended = false
    }, 0)
    return () => {
      // Cleanup: if a new effect run supersedes us (e.g. fast file
      // switching), cancel the pending release and release now.
      clearTimeout(releaseTimer)
      isSettingContent.current = false
      _headingEnforcerSuspended = false
    }
  }, [initialContent])

  return (
    <div className={cn('h-full relative', className)} onBlur={handleBlur}>
      {isDirty && (
        <span className="absolute top-2 right-3 text-xs text-muted-foreground z-10">Unsaved</span>
      )}
      <CodeMirror
        ref={(instance) => {
          ref.current = instance
          viewRef.current = instance?.view ?? null
        }}
        value={initialContent}
        basicSetup={{
          lineNumbers: false,
          highlightActiveLine: true,
          highlightActiveLineGutter: false,
          foldGutter: true,
          foldKeymap: true,
          autocompletion: false, // we provide our own
          syntaxHighlighting: true,
          bracketMatching: true,
          closeBrackets: false,
          defaultKeymap: true,
          history: true,
        }}
        extensions={[
          history(),
          bracketMatching(),
          syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
          customKeymap,
          headingEnforcer,
          linkClickHandler,
          autocompletion({
            override: [completionSource],
            activateOnTyping: true,
            closeOnBlur: true,
          }),
          markdown({ base: markdownLanguage, codeLanguages: languages }),
          baseTheme,
          mermaidExtension,
          mermaidDarkObserver,
          tokenHideExtension,
          wikilinkExtension,
          ...(isDark ? [oneDark, darkTheme] : []),
        ]}
        theme={isDark ? 'dark' : 'light'}
        onChange={(value) => {
          if (isSettingContent.current) return
          setIsDirty(true)
          if (saveTimerRef.current) clearTimeout(saveTimerRef.current)
          saveTimerRef.current = setTimeout(() => {
            onSaveRef.current(value)
            setIsDirty(false)
          }, 1000)
        }}
        height="100%"
        className="h-full hypermd-container"
      />
    </div>
  )
}
