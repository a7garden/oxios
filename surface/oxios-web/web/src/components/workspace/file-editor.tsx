import { useCallback, useEffect, useRef } from 'react'
import {
  EditorView,
  keymap,
  lineNumbers,
  highlightActiveLineGutter,
  highlightSpecialChars,
  drawSelection,
  rectangularSelection,
  crosshairCursor,
  highlightActiveLine,
} from '@codemirror/view'
import { EditorState } from '@codemirror/state'
import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands'
import {
  syntaxHighlighting,
  defaultHighlightStyle,
  bracketMatching,
  foldGutter,
  indentOnInput,
  foldKeymap,
} from '@codemirror/language'
import { oneDark } from '@codemirror/theme-one-dark'
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

  return (
    <div
      ref={containerRef}
      className="h-full w-full overflow-hidden"
    />
  )
}
