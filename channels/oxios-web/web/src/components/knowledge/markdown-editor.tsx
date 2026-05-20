import { useEffect, useRef, useState, useCallback } from 'react'
import CodeMirror from 'codemirror'
import '@/lib/hypermd-setup' // side-effect: registers all CM5/HyperMD modules
import { createLinkHintFn, buildAutocompleteDict } from '@/lib/autocomplete-link'
import { useKnowledgeStore } from '@/stores/knowledge'
import { useKnowledgeTree } from '@/hooks/use-knowledge'
import { cn } from '@/lib/utils'

interface MarkdownEditorProps {
  filePath: string
  initialContent: string
  onSave: (content: string) => void
  className?: string
}

export function MarkdownEditor({ filePath, initialContent, onSave, className }: MarkdownEditorProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const editorRef = useRef<CodeMirror.Editor | null>(null)
  const [isDirty, setIsDirty] = useState(false)
  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined)
  const isSettingContent = useRef(false)
  const openFile = useKnowledgeStore((s) => s.openFile)
  const { data: treeEntries } = useKnowledgeTree()
  const currentFilePath = useKnowledgeStore((s) => s.currentFilePath)

  // Build autocomplete entries from tree
  const autocompleteEntries = useCallback(() => {
    if (!treeEntries) return []
    return buildAutocompleteDict(treeEntries, undefined, currentFilePath ?? undefined)
  }, [treeEntries, currentFilePath])

  // Create editor instance
  useEffect(() => {
    if (!containerRef.current) return

    // Clean up previous instance
    if (editorRef.current) {
      ;(editorRef.current as any).toTextArea?.()
      editorRef.current = null
    }

    const textarea = document.createElement('textarea')
    textarea.value = initialContent
    containerRef.current.appendChild(textarea)

    // Custom link resolver
    const resolveURL = (url: string | undefined): string | undefined => {
      if (!url) return url
      url = url.replace(/%20/g, ' ')
      if (url.startsWith('../')) url = url.replace('../', '')

      // External URLs
      if (/^https?:\/\//i.test(url)) return url

      // .md files → open in editor
      if (/\.md$/i.test(url)) {
        // Defer to avoid CM internal state issues
        setTimeout(() => openFile(url), 0)
        return url
      }

      return url
    }

    // Custom link reader (handles wiki-style click)
    const readLink = (text: string, _line: number) => {
      text = text.replace(/\|.*]$/, '').replace(/[\[\]]/g, '')

      // Action links
      if (text === 'cmd:openDir' || text === 'cmd:openChat') return undefined

      // External URLs
      if (/^https?:\/\//.test(text)) {
        window.open(text, '_blank')
        return
      }

      // Internal .md links
      const path = text.endsWith('.md') ? text : text + '.md'
      setTimeout(() => openFile(path), 0)
      return
    }

    // CodeMirror is registered globally by hypermd-setup side-effect import
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const CM = (window as any).CodeMirror as typeof CodeMirror
    const cm = CM.fromTextArea(textarea, {
      mode: { name: 'hypermd', math: false },
      lineNumbers: false,
      dragDrop: false,
      viewportMargin: 10,
      hmdClick: true,
      styleActiveLine: true,
      extraKeys: {
        'Cmd-B': toggleBold,
        'Ctrl-B': toggleBold,
        'Cmd-I': toggleItalic,
        'Ctrl-I': toggleItalic,
        'Cmd-Y': insertCheckmark,
        'Ctrl-Y': insertCheckmark,
      },
      hintOptions: {
        hint: createLinkHintFn(autocompleteEntries),
        closeCharacters: /[$^]/,
        closeOnUnfocus: false,
        completeSingle: false,
        alignWithWord: false,
      },
    } as any)

    // Override URL resolution and link reading
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    ;(cm as any).hmdResolveURL = resolveURL
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    ;(cm as any).hmdReadLink = readLink

    // Auto-show hints on `[` press
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    cm.on('inputRead', (_cm: any, change: any) => {
      if (change.text.length === 1 && change.text[0] === '[') {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        ;(cm as any).showHint({
          completeSingle: false,
          updateOnCursorActivity: true,
        })
      }
    })

    // Force `# ` on first line
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    cm.on('change', (instance: any, change: any) => {
      if (change.from.line === 0) {
        const line = instance.getLine(0)
        if (line && !line.startsWith('# ')) {
          const content = line.replace(/^#*\s*/, '')
          instance.replaceRange('# ' + content, { line: 0, ch: 0 }, { line: 0, ch: line.length })
        }
      }
    })

    // Change handler for auto-save
    cm.on('change', () => {
      if (isSettingContent.current) return
      setIsDirty(true)

      if (saveTimerRef.current) clearTimeout(saveTimerRef.current)
      saveTimerRef.current = setTimeout(() => {
        const content = cm.getValue()
        onSave(content)
        setIsDirty(false)
      }, 1000)
    })

    // Size
    cm.setSize(null, '100%')
    editorRef.current = cm

    return () => {
      if (saveTimerRef.current) clearTimeout(saveTimerRef.current)
      cm.toTextArea()
      editorRef.current = null
    }
  }, [filePath]) // Re-create on file change

  // Update content when initialContent changes (file loaded from API)
  useEffect(() => {
    const cm = editorRef.current
    if (!cm) return
    const current = cm.getValue()
    if (current !== initialContent) {
      isSettingContent.current = true
      cm.setValue(initialContent)
      isSettingContent.current = false
      cm.clearHistory()
      cm.setCursor({ line: 0, ch: 0 })
    }
  }, [initialContent])

  // Save on blur
  const handleBlur = useCallback(() => {
    const cm = editorRef.current
    if (!cm || !isDirty) return
    if (saveTimerRef.current) clearTimeout(saveTimerRef.current)
    onSave(cm.getValue())
    setIsDirty(false)
  }, [isDirty, onSave])

  // Listen for manual save event from toolbar (⌘S / save button)
  useEffect(() => {
    const handler = () => {
      const cm = editorRef.current
      if (!cm) return
      if (saveTimerRef.current) clearTimeout(saveTimerRef.current)
      onSave(cm.getValue())
      setIsDirty(false)
    }
    document.addEventListener('knowledge:save', handler)
    return () => document.removeEventListener('knowledge:save', handler)
  }, [onSave])

  return (
    <div className={cn('h-full relative', className)} onBlur={handleBlur}>
      {isDirty && (
        <span className="absolute top-2 right-3 text-xs text-muted-foreground z-10">Unsaved</span>
      )}
      <div ref={containerRef} className="h-full hypermd-container" />
    </div>
  )
}

// ── Formatting helpers ────────────────────────────────────────

function toggleBold(cm: CodeMirror.Editor) {
  wrapSelection(cm, '**', '**')
}

function toggleItalic(cm: CodeMirror.Editor) {
  wrapSelection(cm, '*', '*')
}

function insertCheckmark(cm: CodeMirror.Editor) {
  const cursor = cm.getCursor()
  cm.replaceRange('✅ ', { line: cursor.line, ch: 0 })
  cm.focus()
}

function wrapSelection(cm: CodeMirror.Editor, before: string, after: string) {
  const selections = cm.listSelections()
  if (selections.length === 0) return

  cm.replaceSelections(
    selections.map((sel) => {
      const text = cm.getRange(sel.anchor, sel.head)
      return before + text + after
    }),
    'around',
  )
  cm.focus()
}
