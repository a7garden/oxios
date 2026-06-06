import {
  bracketMatching,
  defaultHighlightStyle,
  foldGutter,
  indentOnInput,
  syntaxHighlighting,
} from '@codemirror/language'
import { EditorState } from '@codemirror/state'
import { oneDark } from '@codemirror/theme-one-dark'
import {
  drawSelection,
  EditorView,
  highlightActiveLineGutter,
  highlightSpecialChars,
  lineNumbers,
} from '@codemirror/view'
import { useEffect, useRef } from 'react'
import { getLanguageExtension } from '@/lib/cm6-language'

interface FileViewerProps {
  path: string
  content: string
}

export function FileViewer({ path, content }: FileViewerProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const viewRef = useRef<EditorView | null>(null)

  useEffect(() => {
    if (!containerRef.current) return

    // Destroy previous instance
    viewRef.current?.destroy()

    const langExt = getLanguageExtension(path)

    const extensions = [
      lineNumbers(),
      highlightActiveLineGutter(),
      highlightSpecialChars(),
      drawSelection(),
      EditorState.readOnly.of(true),
      EditorView.editable.of(false),
      syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
      bracketMatching(),
      foldGutter(),
      indentOnInput(),
      oneDark,
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
  }, [path, content])

  return <div ref={containerRef} className="h-full w-full overflow-hidden" />
}
