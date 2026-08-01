/**
 * Oxios knowledge-base markdown editor — CodeMirror 6 via @atomic-editor/editor.
 *
 * Replaces the previous @uiw/react-codemirror-based setup with a direct
 * AtomicCodeMirrorEditor wrapper. Inline preview, WYSIWYG tables, image
 * blocks, edit helpers, and search are provided by atomic-editor's built-in
 * extensions. Oxios-specific features (frontmatter properties, heading
 * enforcement, emoji/math/mermaid folds, heading colors, custom keymap,
 * wikilink resolution against the knowledge tree, stats tracking) are layered
 * as CM6 extensions via the `extensions` prop.
 *
 * Toggleable features (emojiFold, mathFold, mermaidFold) are encoded in the
 * `documentId` so changing them triggers a clean remount. This loses undo
 * history and scroll position; acceptable since these toggles are infrequent.
 */
import {
  AtomicCodeMirrorEditor,
  type AtomicCodeMirrorEditorHandle,
  wikiLinks,
} from '@atomic-editor/editor'
import { ATOMIC_CODE_LANGUAGES } from '@atomic-editor/editor/code-languages'

// Wiki-link types matching atomic-editor's WikiLinksConfig callbacks.
// Not re-exported from @atomic-editor/editor index, so defined locally.
interface WikiLinkSuggestion {
  target: string
  label: string
  detail?: string
  boost?: number
}
interface WikiLinkResolvedTarget {
  target: string
  label: string
  status?: 'resolved' | 'missing' | 'unresolved'
}

import '@atomic-editor/editor/styles.css'
import {
  autocompletion,
  type Completion,
  type CompletionContext,
  type CompletionResult,
} from '@codemirror/autocomplete'
import { HighlightStyle, syntaxHighlighting } from '@codemirror/language'
import { type Extension, Prec } from '@codemirror/state'
import { EditorView, keymap } from '@codemirror/view'
import { tags as lmTags } from '@lezer/highlight'
import { type CSSProperties, useCallback, useEffect, useMemo, useRef } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { api } from '@/lib/api-client'
import { useKnowledgeRecursiveTree } from '@/hooks/use-knowledge'
import { emojiFoldExtension } from '@/lib/emoji-fold-extension'
import { EMOJI_SHORTCODES } from '@/lib/emoji-shortcodes'
import { findFrontmatterRange } from '@/lib/frontmatter'
import { frontmatterExtension } from '@/lib/frontmatter-extension'
import { mathFoldExtension } from '@/lib/math-fold-extension'
import { mermaidDarkObserver, mermaidExtension } from '@/lib/mermaid-extension'
import { cn } from '@/lib/utils'
import { buildWikilinkIndex, resolveWikilink, type WikilinkIndex } from '@/lib/wikilink-resolve'
import { useEditorPrefs } from '@/stores/editor-prefs'
import { useKnowledgeStore } from '@/stores/knowledge'
import { countWords, type EditorStats } from './editor-status-bar'

// ── Props ───────────────────────────────────────────────────────────────

export interface MarkdownEditorProps {
  filePath: string
  initialContent: string
  onSave: (content: string) => Promise<void>
  className?: string
  onStatsChange?: (stats: EditorStats | null) => void
}

// ── Image URL helpers ───────────────────────────────────────────────────
// Resolve relative image URLs to backend asset routes so images render.
// Reversed on save so the backend stores portable relative paths.

const ASSET_ROUTE = '/api/knowledge/asset'
const UNIFIED_ASSET_ROUTE = '/api/assets/'

function isAbsoluteUrl(url: string): boolean {
  return (
    /^(https?:|data:|blob:|about:)/.test(url) ||
    url.startsWith(ASSET_ROUTE) ||
    url.startsWith(UNIFIED_ASSET_ROUTE)
  )
}

function resolveRelativeImages(md: string, fileDir: string): string {
  return md.replace(/!\[([^\]]*)\]\(([^)]+)\)/g, (_match, alt: string, url: string) => {
    if (isAbsoluteUrl(url)) return _match
    const resolved = url.startsWith('/')
      ? `${ASSET_ROUTE}${url}`
      : `${ASSET_ROUTE}/${fileDir}/${url}`
    return `![${alt}](${resolved})`
  })
}

function stripResolvedImages(md: string, fileDir: string): string {
  const prefix = `${ASSET_ROUTE}/`
  const escapedPrefix = prefix.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  return md.replace(
    new RegExp(`!\\[([^\\]]*)\\(${escapedPrefix}([^)]+)\\)`, 'g'),
    (_match: string, alt: string, assetPath: string) => {
      // If assetPath starts with fileDir/, the original was relative (no leading slash)
      if (fileDir && assetPath.startsWith(`${fileDir}/`)) {
        return `![${alt}](${assetPath.slice(fileDir.length + 1)})`
      }
      // Otherwise, the original was root-absolute: /img/logo.png -> /api/knowledge/asset/img/logo.png
      return `![${alt}](/${assetPath})`
    },
  )
}

// ── Wikilink adapter ────────────────────────────────────────────────────
// Maps the Oxios knowledge tree index to atomic-editor's wikiLinks() API.

function resolveWikilinkTarget(
  target: string,
  currentPath: string | null,
  index: WikilinkIndex | null,
): WikiLinkResolvedTarget | null {
  if (!index) return { target, label: target, status: 'missing' }
  const path = resolveWikilink(target, currentPath, index)
  if (path) return { target: path, label: target, status: 'resolved' }
  return { target, label: target, status: 'missing' }
}

function suggestWikilinks(query: string, index: WikilinkIndex | null): WikiLinkSuggestion[] {
  if (!index || !query) return []
  const lower = query.toLowerCase()
  const results: WikiLinkSuggestion[] = []
  for (const [stem, paths] of index) {
    if (stem.includes(lower)) {
      const firstPath = paths[0]
      if (firstPath) results.push({ target: firstPath, label: stem, detail: firstPath })
    }
  }
  return results.slice(0, 12)
}

// ── Emoji completion source ────────────────────────────────────────────

function emojiCompletionSource(context: CompletionContext): CompletionResult | null {
  const match = context.matchBefore(/:([a-z0-9_+]*)$/)
  if (!match || (match.from === match.to && !context.explicit)) return null
  const query = match.text.slice(1).toLowerCase()
  const options = Object.entries(EMOJI_SHORTCODES)
    .filter(([code]) => code.includes(query))
    .slice(0, 10)
    .map(([code, emoji]) => ({
      label: `${emoji}  :${code}:`,
      type: 'keyword' as const,
      apply: `:${code}:`,
    }))
  if (options.length === 0) return null
  return { from: match.from, to: context.pos, options }
}

// ── Wiki-link completion source ──────────────────────────────────────
// atomic-editor's wikiLinks() bundles its own autocompletion(), and
// CodeMirror's completion facet has no merge combiner for the `override`
// field — a second autocompletion() with override throws
// "Config merge conflict for field override". So we own ONE combined
// autocompletion (emoji + wikilinks) here and call wikiLinks() without
// `suggest` so it skips its bundled completion (decorations/resolver/click
// handling are independent extensions and keep working). Mirrors
// atomic-editor's completionSource/serialize logic so the inserted
// `[[target|label]]` resolves correctly.
const WIKI_LINK_QUERY_RE = /\[\[[^\]\n|]*$/

function makeWikiLinkCompletionSource(indexRef: {
  current: WikilinkIndex | null
}): (context: CompletionContext) => CompletionResult | null {
  return (context) => {
    const match = context.matchBefore(WIKI_LINK_QUERY_RE)
    if (!match || (match.from === match.to && !context.explicit)) return null
    const query = match.text.slice(2)
    const suggestions = suggestWikilinks(query, indexRef.current)
    if (suggestions.length === 0) return null
    return {
      from: match.from + 2,
      to: context.pos,
      options: suggestions.map((s) => ({
        label: s.label,
        detail: s.detail,
        type: 'text' as const,
        apply: (view: EditorView, _completion: Completion, from: number, to: number) => {
          // Escape wiki-link delimiters so `]`/`|` in a label can't break the link.
          const label = s.label
            .replaceAll(']', ' ')
            .replaceAll('|', ' ')
            .replace(/\s+/g, ' ')
            .trim()
          const insert = `${s.target}|${label}]]`
          const replaceTo = view.state.doc.sliceString(to, to + 2) === ']]' ? to + 2 : to
          view.dispatch({
            changes: { from, to: replaceTo, insert },
            selection: { anchor: from + insert.length },
          })
        },
      })),
      validFor: /^[^\]\n|]*$/,
    }
  }
}

// ── Heading enforcer ────────────────────────────────────────────────────
// Keep the first content line (after frontmatter) as `# <title>`.
// No WeakSet suspension needed — atomic-editor has no value-prop echo.

const headingEnforcer = EditorView.updateListener.of((update) => {
  if (!update.docChanged || !update.view) return
  const state = update.state
  const doc = state.doc
  const text = state.sliceDoc(0, Math.min(state.doc.length, 8192))
  const fm = findFrontmatterRange(text)
  if (!fm && doc.line(1).text.trimEnd() === '---') return
  const fmEndLine = fm ? doc.lineAt(Math.max(0, fm.to - 1)).number : 0
  const titleLineNum = fmEndLine + 1
  if (titleLineNum > doc.lines) return
  const titleLine = doc.line(titleLineNum)
  if (titleLine.text.startsWith('# ')) return
  update.view.dispatch({
    changes: { from: titleLine.from, to: titleLine.from, insert: '# ' },
  })
})

// ── Custom keymap ──────────────────────────────────────────────────────

function makeKeymap(onSave: () => void) {
  return Prec.highest(
    keymap.of([
      {
        key: 'Mod-s',
        run: () => {
          onSave()
          return true
        },
      },
    ]),
  )
}

// ── Stats tracker ──────────────────────────────────────────────────────

function makeStatsTracker(onStats: ((s: EditorStats) => void) | undefined) {
  const ref = { current: onStats }
  ref.current = onStats
  return EditorView.updateListener.of((update) => {
    if (!update.docChanged && !update.selectionSet) return
    const { state } = update.view
    const text = state.doc.toString()
    const sel = state.selection.main
    const line = state.doc.lineAt(sel.head)
    ref.current?.({
      words: countWords(text),
      chars: text.length,
      lines: state.doc.lines,
      cursorLine: line.number,
      cursorCol: sel.head - line.from + 1,
    })
  })
}

// ── Component ───────────────────────────────────────────────────────────

export function MarkdownEditor({
  filePath,
  initialContent,
  onSave,
  className,
  onStatsChange,
}: MarkdownEditorProps) {
  const { t } = useTranslation()
  const editorRef = useRef<AtomicCodeMirrorEditorHandle | null>(null)
  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined)
  const onSaveRef = useRef(onSave)
  onSaveRef.current = onSave
  const containerRef = useRef<HTMLDivElement | null>(null)

  // ── Image paste/drop → upload to asset store, insert markdown ────
  const insertMarkdownAtCursor = useCallback((md: string) => {
    const cmDom = containerRef.current?.querySelector('.cm-editor')
    if (!cmDom) return
    const view = EditorView.findFromDOM(cmDom as HTMLElement)
    if (!view) return
    const sel = view.state.selection.main
    view.dispatch({
      changes: { from: sel.from, insert: md },
      selection: { anchor: sel.from + md.length },
    })
  }, [])

  const handleImageFiles = useCallback(
    async (files: FileList | File[]) => {
      for (const file of Array.from(files)) {
        if (!file.type.startsWith('image/')) continue
        const fd = new FormData()
        fd.append('file', file)
        fd.append('source', 'editor-paste')
        try {
          const asset = await api.upload<{ url: string; storage_name: string }>(
            '/api/assets',
            fd,
          )
          const alt = file.name.replace(/\.[^.]+$/, '')
          insertMarkdownAtCursor(`![${alt}](${asset.url})\n`)
        } catch {
          toast.error('Failed to upload image')
        }
      }
    },
    [insertMarkdownAtCursor],
  )

  const handlePaste = useCallback(
    (e: React.ClipboardEvent) => {
      if (e.clipboardData.files.length > 0) {
        e.preventDefault()
        handleImageFiles(e.clipboardData.files)
      }
    },
    [handleImageFiles],
  )

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      if (e.dataTransfer.files.length > 0 && Array.from(e.dataTransfer.files).some((f) => f.type.startsWith('image/'))) {
        e.preventDefault()
        handleImageFiles(e.dataTransfer.files)
      }
    },
    [handleImageFiles],
  )
  const openFile = useKnowledgeStore((s) => s.openFile)
  const currentFilePath = useKnowledgeStore((s) => s.currentFilePath)
  const prefs = useEditorPrefs()

  // File directory for image URL resolution
  const filePathRef = useRef(filePath)
  filePathRef.current = filePath
  const getFileDir = useCallback(() => {
    const p = filePathRef.current ?? ''
    const i = p.lastIndexOf('/')
    return i >= 0 ? p.slice(0, i) : ''
  }, [])

  // Pre-resolve relative image URLs so they render in the editor
  const resolvedContent = useMemo(
    () => resolveRelativeImages(initialContent, getFileDir()),
    [initialContent, getFileDir],
  )

  // ── Wikilink index from knowledge tree ──────────────────────────────
  const { data: recursiveTree } = useKnowledgeRecursiveTree()
  const wikilinkIndex = useMemo(
    () => (recursiveTree ? buildWikilinkIndex(recursiveTree) : null),
    [recursiveTree],
  )
  // Ref to latest index so the (mount-captured) wiki-links resolver reads
  // current data on each call without needing a remount.
  const wikilinkIndexRef = useRef(wikilinkIndex)
  wikilinkIndexRef.current = wikilinkIndex

  const wikiLinkConfig = useMemo(
    () => ({
      resolve: async (target: string) =>
        resolveWikilinkTarget(target, currentFilePath, wikilinkIndexRef.current),
      onOpen: (target: string) => openFile(target),
    }),
    [currentFilePath, openFile],
  )

  // ── Heading colors ──────────────────────────────────
  const headingStyleExt = useMemo(() => {
    const headingTagMap = {
      h1: lmTags.heading1,
      h2: lmTags.heading2,
      h3: lmTags.heading3,
      h4: lmTags.heading4,
      h5: lmTags.heading5,
      h6: lmTags.heading6,
    } as const
    const entries: { tag: typeof lmTags.heading1; color: string }[] = []
    for (const lvl of ['h1', 'h2', 'h3', 'h4', 'h5', 'h6'] as const) {
      if (prefs.headingColorsEnabled) {
        const c = prefs.headingColors[lvl]
        if (c) entries.push({ tag: headingTagMap[lvl], color: c })
      } else {
        entries.push({ tag: headingTagMap[lvl], color: 'var(--foreground)' })
      }
    }
    return syntaxHighlighting(HighlightStyle.define(entries))
  }, [prefs.headingColors, prefs.headingColorsEnabled])

  // ── Marker / link color ─────────────────────────────────────────────
  const markerStyleExt = useMemo(() => {
    const exts: Extension[] = []
    if (prefs.markerColor) {
      exts.push(
        syntaxHighlighting(
          HighlightStyle.define([{ tag: lmTags.processingInstruction, color: prefs.markerColor }]),
        ),
      )
    }
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
    return exts
  }, [prefs.markerColor, prefs.linkColor])

  // ── Stats tracker ───────────────────────────────────────────────────
  const statsTracker = useMemo(() => makeStatsTracker(onStatsChange), [])

  // ── Keymap ──────────────────────────────────────────────────────────
  const handleSave = useCallback(() => {
    document.dispatchEvent(new CustomEvent('knowledge:save'))
  }, [])
  const keymapExt = useMemo(() => makeKeymap(handleSave), [handleSave])

  // ── Manual save handler (⌘S / toolbar) ──────────────────────────────
  useEffect(() => {
    const handler = async () => {
      const md = editorRef.current?.getMarkdown()
      if (!md) return
      clearTimeout(saveTimerRef.current)
      try {
        const cleaned = stripResolvedImages(md, getFileDir())
        await onSaveRef.current(cleaned)
      } catch {
        toast.error(t('knowledge.saveFailed'))
      }
    }
    document.addEventListener('knowledge:save', handler)
    return () => {
      clearTimeout(saveTimerRef.current)
      document.removeEventListener('knowledge:save', handler)
    }
  }, [t])

  // ── External open-file listener (from link click) ───────────────────
  useEffect(() => {
    const handler = (e: Event) => {
      const detail = (e as CustomEvent<{ path: string }>).detail
      if (detail?.path) openFile(detail.path)
    }
    document.addEventListener('knowledge:open-file', handler)
    return () => document.removeEventListener('knowledge:open-file', handler)
  }, [openFile])

  // ── Title edit handler (from NoteTitle) ─────────────────────────────
  // Responds to knowledge:set-title events, rewrites the H1 line, saves.
  useEffect(() => {
    const handler = async (e: Event) => {
      const detail = (e as CustomEvent<{ path: string; title: string }>).detail
      if (!detail?.path || detail.path !== filePath || !detail.title) return
      // Access the view through the content DOM
      const contentDOM = editorRef.current?.getContentDOM()
      if (!contentDOM) return
      const view = EditorView.findFromDOM(contentDOM)
      if (!view) return

      const { state } = view
      const doc = state.doc.toString()
      const fm = findFrontmatterRange(doc)
      if (!fm && state.doc.line(1).text.trimEnd() === '---') return
      const fmEndLine = fm ? state.doc.lineAt(Math.max(0, fm.to - 1)).number : 0
      const titleLineNum = fmEndLine + 1
      if (titleLineNum > state.doc.lines) return
      const titleLine = state.doc.line(titleLineNum)

      view.dispatch({
        changes: { from: titleLine.from, to: titleLine.to, insert: `# ${detail.title}` },
      })

      // Save immediately
      clearTimeout(saveTimerRef.current)
      try {
        const next = view.state.doc.toString()
        const cleaned = stripResolvedImages(next, getFileDir())
        await onSaveRef.current(cleaned)
      } catch {
        toast.error(t('knowledge.saveFailed'))
      }
    }
    document.addEventListener('knowledge:set-title', handler)
    return () => document.removeEventListener('knowledge:set-title', handler)
  }, [filePath])

  // ── Debounced onMarkdownChange ──────────────────────────────────────
  const handleChange = useCallback(
    (md: string) => {
      clearTimeout(saveTimerRef.current)
      saveTimerRef.current = setTimeout(async () => {
        try {
          const cleaned = stripResolvedImages(md, getFileDir())
          await onSaveRef.current(cleaned)
        } catch {
          toast.error(t('knowledge.saveFailed'))
        }
      }, 1000)
    },
    [getFileDir],
  )

  // ── Build extensions array ─────────────────────────────────────────
  const extensions = useMemo<Extension[]>(() => {
    // Single combined autocompletion: two autocompletion() calls both
    // setting `override` throw "Config merge conflict for field override"
    // (CM6 completion facet has no override merge combiner). wikiLinks() is
    // called without `suggest` above so it skips its bundled completion.
    const wikiLinkSource = makeWikiLinkCompletionSource(wikilinkIndexRef)
    const exts: Extension[] = [
      frontmatterExtension,
      headingEnforcer,
      keymapExt,
      statsTracker,
      wikiLinks(wikiLinkConfig),
      headingStyleExt,
      ...markerStyleExt,
      autocompletion({
        override: [emojiCompletionSource, wikiLinkSource],
        activateOnTyping: true,
        closeOnBlur: true,
      }),
    ]
    // Toggleable Oxios fold extensions
    if (prefs.emojiFold) exts.push(emojiFoldExtension)
    if (prefs.mathFold) exts.push(mathFoldExtension)
    if (prefs.mermaidFold) {
      exts.push(mermaidExtension)
      exts.push(mermaidDarkObserver)
    }
    return exts
  }, [
    keymapExt,
    statsTracker,
    wikiLinkConfig,
    headingStyleExt,
    markerStyleExt,
    prefs.emojiFold,
    prefs.mathFold,
    prefs.mermaidFold,
  ])

  // Extensions are captured at mount by atomic-editor. To make toggleable
  // prefs reactive, encode them in documentId so changing a fold pref
  // triggers a clean remount.
  const foldKey = `${prefs.emojiFold ? 'E' : ''}${prefs.mathFold ? 'M' : ''}${prefs.mermaidFold ? 'D' : ''}`
  const documentId = foldKey ? `${filePath}::fold:${foldKey}` : filePath

  return (
    <div
      ref={containerRef}
      onPaste={handlePaste}
      onDrop={handleDrop}
      className={cn('ox-knowledge-editor h-full relative', className)}
      style={
        {
          '--atomic-editor-fg': 'var(--foreground)',
          '--atomic-editor-fg-muted': 'var(--muted-foreground)',
          '--atomic-editor-fg-faint': 'var(--muted-foreground)',
          '--atomic-editor-bg': 'transparent',
          '--atomic-editor-accent': 'var(--primary)',
          '--atomic-editor-accent-bright': 'var(--primary)',
          '--atomic-editor-link': 'var(--primary)',
          '--atomic-editor-link-hover': 'var(--primary-foreground)',
          '--atomic-editor-selection-bg': 'var(--accent)',
          '--atomic-editor-code-bg': 'var(--muted)',
          '--atomic-editor-border': 'var(--border)',
          '--atomic-editor-bg-surface': 'var(--card)',
          '--atomic-editor-bg-panel': 'var(--card)',
          '--atomic-editor-body-size': `${prefs.fontSize}px`,
          '--atomic-editor-font': prefs.fontFamily,
          '--atomic-editor-body-leading': String(prefs.lineHeight),
          '--atomic-editor-measure': '100%',
          '--atomic-editor-font-mono': 'ui-monospace, SFMono-Regular, Menlo, Monaco, monospace',
        } as CSSProperties
      }
    >
      <AtomicCodeMirrorEditor
        documentId={documentId}
        markdownSource={resolvedContent}
        onMarkdownChange={handleChange}
        onLinkClick={(url) => window.open(url, '_blank', 'noopener,noreferrer')}
        editorHandleRef={editorRef}
        codeLanguages={ATOMIC_CODE_LANGUAGES}
        extensions={extensions}
      />
    </div>
  )
}
