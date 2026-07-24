// Mermaid renderer — compiles diagram source to SVG via the mermaid package.
//
// Reuses the already-bundled `mermaid` dep (also used by the knowledge editor
// CodeMirror extension). This component is itself code-split (lazy-loaded by
// the dispatcher), so the static import lands in a dedicated chunk that is only
// fetched when a mermaid artifact is actually opened. Theme follows the app
// dark/light mode.

import mermaid from 'mermaid'
import { useTheme } from 'next-themes'
import { memo, useEffect, useId, useState } from 'react'
import { useTranslation } from 'react-i18next'

interface MermaidRendererProps {
  content: string
}

export const MermaidRenderer = memo(function MermaidRenderer({ content }: MermaidRendererProps) {
  const { resolvedTheme } = useTheme()
  const { t } = useTranslation()
  const [svg, setSvg] = useState('')
  const [error, setError] = useState<string | null>(null)
  // useId is session-stable; strip ':' to keep mermaid render ids valid.
  const rawId = useId().replace(/[:]/g, '')

  useEffect(() => {
    let cancelled = false
    setError(null)
    renderMermaid(`${rawId}-m`, content, resolvedTheme === 'dark' ? 'dark' : 'default')
      .then((out) => {
        if (!cancelled) setSvg(out)
      })
      .catch((e: unknown) => {
        if (!cancelled) setError(e instanceof Error ? e.message : String(e))
      })
    return () => {
      cancelled = true
    }
  }, [content, rawId, resolvedTheme])

  if (error) {
    return (
      <div className="flex h-full w-full items-center justify-center p-4 text-sm text-destructive">
        {t('artifact.mermaidError')}: {error}
      </div>
    )
  }

  return (
    <div
      className="flex h-full w-full items-center justify-center overflow-auto p-4"
      // eslint-disable-next-line react/no-danger -- mermaid renders to trusted SVG
      dangerouslySetInnerHTML={{ __html: svg }}
    />
  )
})

async function renderMermaid(id: string, text: string, theme: 'dark' | 'default'): Promise<string> {
  // securityLevel: 'strict' disables click-binding script injection in diagrams.
  mermaid.initialize({ startOnLoad: false, securityLevel: 'strict', theme })
  const { svg } = await mermaid.render(id, text)
  return svg
}
