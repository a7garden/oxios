// ArtifactCard — inline trigger rendered in place of a renderable code block.
//
// Clicking opens (or closes) the side preview panel. While the owning message
// streams and this card is the active artifact, it pushes its live content to
// the store so the panel preview updates in real time.
//
// LobeHub analogue: Conversation/Markdown/plugins/LobeArtifact/Render.

import { Code, Component, Eye, Globe, Image as ImageIcon, Loader2, Workflow } from 'lucide-react'
import { createContext, useContext, useEffect, useMemo } from 'react'
import { useTranslation } from 'react-i18next'
import { cn } from '@/lib/utils'
import { artifactKey, usePortalStore } from '@/stores/portal'
import { ArtifactType, parseArtifactCode } from '@/types/artifact'

/** Context provided by MarkdownMessage so cards know their owning message. */
export interface ArtifactContextValue {
  messageId: string
  isStreaming: boolean
}

export const ArtifactContext = createContext<ArtifactContextValue>({
  messageId: '',
  isStreaming: false,
})

const TYPE_ICON: Record<ArtifactType, typeof Code> = {
  [ArtifactType.Html]: Globe,
  [ArtifactType.Svg]: ImageIcon,
  [ArtifactType.Mermaid]: Workflow,
  [ArtifactType.React]: Component,
}

const TYPE_LABEL: Record<ArtifactType, string> = {
  [ArtifactType.Html]: 'HTML',
  [ArtifactType.Svg]: 'SVG',
  [ArtifactType.Mermaid]: 'Mermaid',
  [ArtifactType.React]: 'React',
}

interface ArtifactCardProps {
  type: ArtifactType
  language?: string
  source: 'language' | 'tag'
  /** Raw code (may include a leading title directive line). */
  children: string
}

export function ArtifactCard({ type, language, source, children }: ArtifactCardProps) {
  const { t } = useTranslation()
  const ctx = useContext(ArtifactContext)

  const parsed = useMemo(() => parseArtifactCode(children), [children])
  const title = parsed.title
  const raw = parsed.raw

  const toggleArtifact = usePortalStore((s) => s.toggleArtifact)
  const updateArtifactContent = usePortalStore((s) => s.updateArtifactContent)

  const meta = { messageId: ctx.messageId, type, title, language, source }
  const key = artifactKey(meta)
  const top = usePortalStore((s) => s.stack[s.stack.length - 1])
  const isActive = top?.type === 'artifact' && top.key === key

  // Live content sync: push the current code to the store while active so the
  // panel preview tracks streaming updates without re-parsing markdown.
  useEffect(() => {
    if (isActive) updateArtifactContent(key, raw)
  }, [isActive, key, raw, updateArtifactContent])

  const Icon = TYPE_ICON[type] ?? Code
  const label = title ?? TYPE_LABEL[type]
  const streaming = ctx.isStreaming && isActive

  const handleToggle = () => toggleArtifact(meta, raw)

  return (
    <button
      type="button"
      onClick={handleToggle}
      aria-label={t('artifact.openPreview')}
      className={cn(
        'group my-3 flex w-full cursor-pointer items-center gap-3 rounded-lg border bg-card px-3 py-2.5 text-left transition-colors hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring',
        isActive && 'border-primary ring-1 ring-primary/30',
      )}
    >
      <div className="flex size-9 shrink-0 items-center justify-center rounded-md bg-muted text-muted-foreground">
        <Icon className="size-4" />
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="truncate text-sm font-medium">{label}</span>
          {streaming && <Loader2 className="size-3 shrink-0 animate-spin text-muted-foreground" />}
        </div>
        <div className="flex items-center gap-1.5 text-xs text-muted-foreground">
          <span>{TYPE_LABEL[type]}</span>
          <span aria-hidden>·</span>
          <span>
            {raw.length.toLocaleString()} {t('artifact.chars')}
          </span>
        </div>
      </div>
      <Eye
        className={cn(
          'size-4 shrink-0 text-muted-foreground transition-opacity',
          isActive ? 'opacity-100' : 'opacity-0 group-hover:opacity-100',
        )}
      />
    </button>
  )
}
