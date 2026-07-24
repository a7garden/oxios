// ArtifactRenderer — type-dispatching renderer for the preview panel.
//
// HTML and SVG renderers are tiny (static import). Mermaid and React are heavy
// (mermaid lib; Sandpack in-browser bundler) so they are lazy-loaded into
// separate chunks, fetched only when that artifact type is actually opened.

import { Loader2 } from 'lucide-react'
import { lazy, Suspense } from 'react'
import { useTranslation } from 'react-i18next'
import { ArtifactType, parseArtifactCode } from '@/types/artifact'
import { HtmlRenderer } from './renderers/html-renderer'
import { SvgRenderer } from './renderers/svg-renderer'

// Named export in mermaid-renderer → wrap to satisfy React.lazy's default shape.
const MermaidRenderer = lazy(() =>
  import('./renderers/mermaid-renderer').then((m) => ({ default: m.MermaidRenderer })),
)
const ReactRenderer = lazy(() => import('./renderers/react-renderer'))

interface ArtifactRendererProps {
  type: ArtifactType
  /** Raw code, possibly with a leading title directive line. */
  code: string
  /** Explicit title (from the card) if already resolved. */
  title?: string
}

function Loading() {
  const { t } = useTranslation()
  return (
    <div className="flex h-full w-full items-center justify-center gap-2 text-sm text-muted-foreground">
      <Loader2 className="size-4 animate-spin" />
      {t('artifact.loading')}
    </div>
  )
}

export function ArtifactRenderer({ type, code, title }: ArtifactRendererProps) {
  // Strip a leading title directive so it never leaks into the rendered output.
  const parsed = parseArtifactCode(code)
  const content = parsed.content
  const resolvedTitle = title ?? parsed.title

  switch (type) {
    case ArtifactType.Html:
      return <HtmlRenderer content={content} />
    case ArtifactType.Svg:
      return <SvgRenderer content={content} />
    case ArtifactType.Mermaid:
      return (
        <Suspense fallback={<Loading />}>
          <MermaidRenderer content={content} />
        </Suspense>
      )
    case ArtifactType.React:
      return (
        <Suspense fallback={<Loading />}>
          <ReactRenderer content={content} title={resolvedTitle} />
        </Suspense>
      )
    default:
      return null
  }
}
