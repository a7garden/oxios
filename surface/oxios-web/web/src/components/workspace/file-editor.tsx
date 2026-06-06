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
import { useCallback, useEffect, useRef } from 'react'
import { getLanguageExtension } from '@/lib/cm6-language'

interface FileEditorProps {
  path: string
  content: string
  onSave: (content: string) => void
  onChange?: (content: string) => void
}

export function FileEditor({ path, content, onSave, onChange }: FileEditorProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const viewRef = useRef<EditorView | null>(null)

  const handleSave = useCallback(() => {
    if (viewRef.current) {
      const doc = viewRef.current.state.doc.toString()
      onSave(doc)
    }
  }, [onSave])

  useEffect(() => {
    if (!containerRef.current) return

    // Destroy previous instance
    viewRef.current?.destroy()

    const langExt = getLanguageExtension(path)

    const saveKeymap = keymap.of([
      {
        key: 'Mod-s',
        run: () => {
          handleSave()
          return true
        },
        preventDefault: true,
      },
    ])

    const updateListener = EditorView.updateListener.of((update) => {
      if (update.docChanged && onChange) {
        onChange(update.state.doc.toString())
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
        '.cm-scroller': { overflow: 'auto' },
      }),
    ]

    if (langExt) {
      extensions.push(langExt)
    }

    const state = EditorState.create({
      doc: content,
      extensions,
    })

    const view = new EditorView({
      state,
      parent: containerRef.current,
    })

    viewRef.current = view

    return () => {
      view.destroy()
      viewRef.current = null
    }
  }, [path, content, handleSave, onChange])

  return <div ref={containerRef} className="h-full w-full overflow-hidden" />
}
