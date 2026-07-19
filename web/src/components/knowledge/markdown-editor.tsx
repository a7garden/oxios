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

import {
  autocompletion,
  type Completion,
  type CompletionContext,
  type CompletionResult,
} from '@codemirror/autocomplete'
import { history, indentWithTab } from '@codemirror/commands'
import { markdown, markdownLanguage } from '@codemirror/lang-markdown'
import {
  bracketMatching,
  defaultHighlightStyle,
  HighlightStyle,
  syntaxHighlighting,
} from '@codemirror/language'
import { languages } from '@codemirror/language-data'
import { EditorSelection, type Extension, Prec } from '@codemirror/state'
import { oneDark } from '@codemirror/theme-one-dark'
import { keymap } from '@codemirror/view'
import { tags as lmTags } from '@lezer/highlight'
import { Strikethrough, Table, TaskList } from '@lezer/markdown'
import CodeMirror, { EditorView, type ReactCodeMirrorRef } from '@uiw/react-codemirror'
import { type CSSProperties, useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { useKnowledgeRecursiveTree, useKnowledgeTree } from '@/hooks/use-knowledge'
import { buildAutocompleteDict, type FileEntry } from '@/lib/autocomplete-link'
import { emojiFoldExtension } from '@/lib/emoji-fold-extension'
import { EMOJI_SHORTCODES } from '@/lib/emoji-shortcodes'
import { createImageFoldExtension } from '@/lib/image-fold-extension'
import { livePreviewExtension, livePreviewHighlight } from '@/lib/live-preview-extension'
import { mathFoldExtension } from '@/lib/math-fold-extension'
import { mermaidDarkObserver, mermaidExtension } from '@/lib/mermaid-extension'
import { tableFoldExtension } from '@/lib/table-fold-extension'
import { tokenHideExtension } from '@/lib/token-hide-extension'
import { cn } from '@/lib/utils'
import { buildWikilinkIndex, resolveWikilink, type WikilinkIndex } from '@/lib/wikilink-resolve'
import { configureWikilinkResolver, wikilinkExtension } from '@/lib/wikilink-extension'
import { useEditorPrefs } from '@/stores/editor-prefs'
import { useKnowledgeStore } from '@/stores/knowledge'
import { countWords, type EditorStats } from './editor-status-bar'

interface MarkdownEditorProps {
  filePath: string
  initialContent: string
  onSave: (content: string) => Promise<void>
  className?: string
  onStatsChange?: (stats: EditorStats) => void
}

// ─────────────────────────────────────────────────────────────────────────
// Custom keymap: ⌘B / Ctrl-B (bold), ⌘I / Ctrl-I (italic), ⌘Y (checklist),
// ⌘S / Ctrl-S (manual save via global 'knowledge:save' event)
// ─────────────────────────────────────────────────────────────────────────
const customKeymap = keymap.of([
  { key: 'Mod-b', run: wrapSelection('**', '**') },
  { key: 'Mod-i', run: wrapSelection('*', '*') },
  { key: 'Mod-y', run: insertCheckmark },
  {
    key: 'Mod-s',
    run: () => {
      document.dispatchEvent(new Event('knowledge:save'))
      return true
    },
  },
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
        changes.map((c) => EditorSelection.range(c.from + before.length, c.to + before.length)),
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
// Gated by a per-EditorView flag so the enforcer does NOT fire while
// we're programmatically replacing the document content (which would
// cause an infinite loop: the enforcer dispatches a change → enforcer
// fires again → …).
//
// Per-view state is tracked via a WeakSet<EditorView>. Using a
// module-level boolean (the previous design) was unsafe if more than
// one MarkdownEditor ever mounted simultaneously — a programmatic
// replacement on view A would suppress the enforcer for view B.
// ─────────────────────────────────────────────────────────────────────────
const _headingEnforcerSuspended = new WeakSet<EditorView>()
const headingEnforcer = EditorView.updateListener.of((update) => {
  if (!update.docChanged) return
  if (_headingEnforcerSuspended.has(update.view)) return
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
function makeCompletionSource(getEntries: () => FileEntry[], emojiDict: Record<string, string>) {
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

// ─────────────────────────────────────────────────────────────────────────
// Link / wiki click handler — same semantics as HyperMD's hmdClick
// ─────────────────────────────────────────────────────────────────────────
const linkClickHandler = EditorView.domEventHandlers({
  click(event, _view) {
    const target = event.target as HTMLElement | null
    if (target?.tagName !== 'A') return false
    if (!(target instanceof HTMLAnchorElement)) return false
    const href = target.getAttribute('href') ?? ''
    if (!href) return false
    if (href.startsWith('http://') || href.startsWith('https://')) {
      window.open(href, '_blank', 'noopener')
      return true
    }
    if (href.startsWith('cmd:')) return true
    const path = href.endsWith('.md') ? href : `${href}.md`
    document.dispatchEvent(new CustomEvent('knowledge:open-file', { detail: { path } }))
    return true
  },
})

// ─────────────────────────────────────────────────────────────────────────
// Editor base theme
// ─────────────────────────────────────────────────────────────────────────
const baseTheme = EditorView.theme({
  '&': {
    fontSize: 'var(--editor-font-size, 0.875rem)',
    height: '100%',
  },
  '.cm-scroller': {
    fontFamily: 'var(--editor-font-mono, ui-monospace, monospace)',
    lineHeight: 'var(--editor-line-height, 1.7)',
  },
  '.cm-content': {
    padding: '12px 8px',
  },
})

// Force the editor canvas transparent so it inherits the app's
// --background token in both themes. Wrapped in Prec.highest to win
// over oneDark (#282c34 bluish canvas) and @uiw's built-in theme,
// regardless of extension order. Syntax-highlight colours are untouched.
const transparentCanvas = Prec.highest(
  EditorView.theme({
    '&': { backgroundColor: 'transparent' },
    '.cm-scroller': { backgroundColor: 'transparent' },
    '.cm-content': { backgroundColor: 'transparent' },
    '.cm-gutters': { backgroundColor: 'transparent' },
  }),
)

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
  onStatsChange,
}: MarkdownEditorProps) {
  const ref = useRef<ReactCodeMirrorRef | null>(null)
  const viewRef = useRef<EditorView | null>(null)
  const [isDirty, setIsDirty] = useState(false)
  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined)
  // B-2: Dirty-state ref — checked by the content-replace effect to
  // prevent our own save echo (setQueryData in onSuccess → new
  // initialContent → effect) from clobbering unsaved edits made
  // during the PUT round-trip.
  const dirtyRef = useRef(false)
  dirtyRef.current = isDirty
  const isSettingContent = useRef(false)
  const openFile = useKnowledgeStore((s) => s.openFile)
  const currentFilePath = useKnowledgeStore((s) => s.currentFilePath)
  const prefs = useEditorPrefs()
  // Image fold needs the current note's directory to resolve relative image
  // URLs against the backend asset route. A ref lets the (stable) extension
  // read the latest path on each rebuild without being recreated.
  const filePathRef = useRef(currentFilePath)
  filePathRef.current = currentFilePath
  const imageFoldExt = useMemo(
    () =>
      createImageFoldExtension(() => {
        const p = filePathRef.current ?? ''
        const i = p.lastIndexOf('/')
        return i >= 0 ? p.slice(0, i) : ''
      }),
    [],
  )
  const { data: treeEntries } = useKnowledgeTree()
  // Recursive tree carries full note paths; used to build the wikilink
  // resolver index. React Query dedupes this with the sidebar's own
  // recursive-tree fetch, so the extra subscription is free.
  const { data: recursiveTree } = useKnowledgeRecursiveTree()
  // Build the stem → paths index whenever the tree changes. Cheap for
  // personal KBs (hundreds of files) and memoized so decoration rebuilds
  // don't re-walk.
  const wikilinkIndex: WikilinkIndex | null = useMemo(
    () => (recursiveTree ? buildWikilinkIndex(recursiveTree) : null),
    [recursiveTree],
  )
  // Install the resolver into the (module-level) wikilink extension. The
  // extension bumps an internal version counter via configureWikilinkResolver
  // and re-resolves every visible link on the next update — so renaming a
  // note makes its inbound `[[links]]` re-bind without an editor remount.
  useEffect(() => {
    if (!wikilinkIndex) {
      configureWikilinkResolver(null)
      return
    }
    const idx = wikilinkIndex
    const path = currentFilePath
    configureWikilinkResolver((target) => resolveWikilink(target, path, idx))
    return () => configureWikilinkResolver(null)
  }, [wikilinkIndex, currentFilePath])
  const [isDark, setIsDark] = useState(false)
  const { t } = useTranslation()

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
  const onStatsChangeRef = useRef(onStatsChange)
  onStatsChangeRef.current = onStatsChange

  const autocompleteEntries = useCallback(() => {
    if (!treeEntries) return []
    return buildAutocompleteDict(treeEntries, undefined, currentFilePath ?? undefined)
  }, [treeEntries, currentFilePath])

  const completionSource = useMemo(
    () => makeCompletionSource(autocompleteEntries, EMOJI_SHORTCODES),
    [autocompleteEntries],
  )

  // Stats tracker — fires on doc/selection changes, reports to parent.
  // Created once (stable identity); reads the latest callback via ref.
  const statsTracker = useMemo(
    () =>
      EditorView.updateListener.of((update) => {
        if (!update.docChanged && !update.selectionSet) return
        const { state } = update.view
        const text = state.doc.toString()
        const sel = state.selection.main
        const line = state.doc.lineAt(sel.head)
        onStatsChangeRef.current?.({
          words: countWords(text),
          chars: text.length,
          lines: state.doc.lines,
          cursorLine: line.number,
          cursorCol: sel.head - line.from + 1,
        })
      }),
    [],
  )

  // ─────────────────────────────────────────────────────────────────────────
  // Conditional extensions — built from editor prefs. Each live-rendering /
  // fold extension can be toggled independently. The relative ordering is
  // preserved from the pre-prefs hard-coded array: syntax-highlighting and
  // transparentCanvas stay last so inline styling wins over oneDark.
  // ─────────────────────────────────────────────────────────────────────────
  const extensions = useMemo<Extension[]>(() => {
    const exts: Extension[] = [
      history(),
      syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
      customKeymap,
      headingEnforcer,
      linkClickHandler,
      autocompletion({
        override: [completionSource],
        activateOnTyping: true,
        closeOnBlur: true,
      }),
      markdown({
        base: markdownLanguage,
        codeLanguages: languages,
        extensions: [Strikethrough, Table, TaskList],
      }),
      baseTheme,
      statsTracker,
    ]
    if (prefs.bracketMatching) exts.push(bracketMatching())
    if (prefs.mermaidFold) {
      exts.push(mermaidExtension)
      exts.push(mermaidDarkObserver)
    }
    if (prefs.tokenHiding) exts.push(tokenHideExtension)
    if (prefs.livePreview) exts.push(livePreviewExtension)
    exts.push(wikilinkExtension)
    if (prefs.emojiFold) exts.push(emojiFoldExtension)
    if (prefs.mathFold) exts.push(mathFoldExtension)
    if (prefs.imageFold) exts.push(imageFoldExt)
    if (prefs.tableFold) exts.push(tableFoldExtension)
    if (isDark) {
      exts.push(oneDark)
      exts.push(darkTheme)
    }
    // Inline live-preview styling — must stay last to win over oneDark.
    exts.push(syntaxHighlighting(livePreviewHighlight))

    // ── User-defined markdown colors (override oneDark / defaults) ──
    // Heading colors via HighlightStyle — oneDark styles `tags.heading`
    // (the parent), so `tags.heading1`-`heading6` (children) override it.
    const headingEntries: { tag: typeof lmTags.heading1; color: string }[] = []
    const headingTagMap = {
      h1: lmTags.heading1,
      h2: lmTags.heading2,
      h3: lmTags.heading3,
      h4: lmTags.heading4,
      h5: lmTags.heading5,
      h6: lmTags.heading6,
    } as const
    for (const lvl of ['h1', 'h2', 'h3', 'h4', 'h5', 'h6'] as const) {
      const c = prefs.headingColors[lvl]
      if (c) headingEntries.push({ tag: headingTagMap[lvl], color: c })
    }
    if (headingEntries.length > 0) {
      exts.push(syntaxHighlighting(HighlightStyle.define(headingEntries)))
    }
    // Markdown syntax markers (`#`, `*`, `` ` ``, `>`)
    if (prefs.markerColor) {
      exts.push(
        syntaxHighlighting(
          HighlightStyle.define([{ tag: lmTags.processingInstruction, color: prefs.markerColor }]),
        ),
      )
    }
    // Links and URLs
    if (prefs.linkColor) {
      exts.push(
        syntaxHighlighting(
          HighlightStyle.define([
            { tag: lmTags.link, color: prefs.linkColor },
            { tag: lmTags.url, color: prefs.linkColor },
          ]),
        ),
      )
    }

    exts.push(transparentCanvas)
    return exts
  }, [
    completionSource,
    imageFoldExt,
    isDark,
    prefs.bracketMatching,
    prefs.mermaidFold,
    prefs.tokenHiding,
    prefs.livePreview,
    prefs.emojiFold,
    prefs.mathFold,
    prefs.imageFold,
    prefs.tableFold,
    statsTracker,
    prefs.headingColors,
    prefs.markerColor,
    prefs.linkColor,
  ])

  // Manual save handler (toolbar / ⌘S)
  useEffect(() => {
    const handler = async () => {
      const view = viewRef.current
      if (!view) return
      clearTimeout(saveTimerRef.current)
      const value = view.state.doc.toString()
      try {
        await onSaveRef.current(value)
        // B-3: Only clear dirty if the editor content hasn't drifted
        // since the save was initiated. If the user typed during the
        // PUT round-trip, dirty stays true so the dirty guard (B-2)
        // blocks the save echo from clobbering their new edits.
        if (viewRef.current?.state.doc.toString() === value) {
          setIsDirty(false)
        }
      } catch {
        // I-1: Save failed — keep dirty so the unsaved indicator
        // stays visible and the user knows their edits are at risk.
        toast.error(t('knowledge.saveFailed'))
      }
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
  const handleBlur = useCallback(async () => {
    const view = viewRef.current
    if (!view || !isDirty) return
    clearTimeout(saveTimerRef.current)
    const value = view.state.doc.toString()
    try {
      await onSaveRef.current(value)
      if (viewRef.current?.state.doc.toString() === value) {
        setIsDirty(false)
      }
    } catch {
      toast.error(t('knowledge.saveFailed'))
    }
  }, [isDirty])

  // Update content when initialContent changes (file loaded from API)
  useEffect(() => {
    const view = viewRef.current
    if (!view) return
    if (dirtyRef.current) return
    const current = view.state.doc.toString()
    if (current === initialContent) return
    // Suspend the heading enforcer and onChange-driven autosave while
    // we programmatically replace the document. Combining the change
    // and the selection reset into a SINGLE dispatch avoids the
    // enforcer firing on the intermediate state (which has the
    // cursor at the end of the old content) and producing unwanted
    // headings or selection drift.
    isSettingContent.current = true
    _headingEnforcerSuspended.add(view)
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
      _headingEnforcerSuspended.delete(view)
    }, 0)
    return () => {
      // Cleanup: if a new effect run supersedes us (e.g. fast file
      // switching), cancel the pending release and release now.
      clearTimeout(releaseTimer)
      isSettingContent.current = false
      _headingEnforcerSuspended.delete(view)
    }
  }, [initialContent])

  // Report initial stats on mount and when content is loaded — the
  // updateListener only fires on *changes*, not on the initial state.
  useEffect(() => {
    const view = viewRef.current
    if (!view) return
    const text = view.state.doc.toString()
    const sel = view.state.selection.main
    const line = view.state.doc.lineAt(sel.head)
    onStatsChangeRef.current?.({
      words: countWords(text),
      chars: text.length,
      lines: view.state.doc.lines,
      cursorLine: line.number,
      cursorCol: sel.head - line.from + 1,
    })
  }, [initialContent])

  return (
    <div
      className={cn('h-full relative', className)}
      onBlur={handleBlur}
      style={
        {
          '--editor-font-size': `${prefs.fontSize}px`,
          '--editor-line-height': String(prefs.lineHeight),
          '--editor-font-mono': prefs.fontFamily,
        } as CSSProperties
      }
    >
      {isDirty && (
        <span className="absolute top-2 right-3 text-xs text-muted-foreground z-10">
          {t('knowledge.unsavedChanges')}
        </span>
      )}
      <CodeMirror
        ref={(instance) => {
          ref.current = instance
          viewRef.current = instance?.view ?? null
        }}
        value={initialContent}
        basicSetup={{
          lineNumbers: prefs.lineNumbers,
          highlightActiveLine: prefs.activeLineHighlight,
          highlightActiveLineGutter: prefs.activeLineHighlight,
          foldGutter: prefs.foldGutter,
          foldKeymap: true,
          autocompletion: false, // we provide our own
          syntaxHighlighting: true,
          bracketMatching: false, // controlled via the extensions array below
          closeBrackets: false,
          defaultKeymap: true,
          history: true,
        }}
        extensions={extensions}
        theme={isDark ? 'dark' : 'light'}
        onChange={(value) => {
          if (isSettingContent.current) return
          setIsDirty(true)
          clearTimeout(saveTimerRef.current)
          saveTimerRef.current = setTimeout(async () => {
            try {
              await onSaveRef.current(value)
              if (viewRef.current?.state.doc.toString() === value) {
                setIsDirty(false)
              }
            } catch {
              toast.error(t('knowledge.saveFailed'))
            }
          }, 1000)
        }}
        height="100%"
        className="h-full hypermd-container"
      />
    </div>
  )
}
