import { useEffect, useRef } from 'react'
import { EditorView, lineNumbers, highlightActiveLineGutter, highlightSpecialChars, drawSelection } from '@codemirror/view'
import { EditorState } from '@codemirror/state'
import { syntaxHighlighting, defaultHighlightStyle, bracketMatching, foldGutter, indentOnInput } from '@codemirror/language'
import { oneDark } from '@codemirror/theme-one-dark'
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

  return (
    <div
      ref={containerRef}
      className="h-full w-full overflow-hidden"
    />
  )
}
