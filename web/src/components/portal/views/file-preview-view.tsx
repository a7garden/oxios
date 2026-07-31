// FilePreviewView — portal view body for previewing a file by path.
//
// LobeHub analogue: features/Portal/FilePreview (header w/ filename + tabs).
// Oxios reuses the shared PortalHeader (path → basename) for the title, so
// this component only owns the body renderer:
//   - .md files         → ReactMarkdown rendering (gfm, syntax highlight)
//   - everything else   → plain scrollable <pre> in monospace
//
// Content is passed in via the PortalView payload. When undefined (still
// loading), we render a small "Loading…" placeholder; the caller streams
// content in via `updateFilePreviewContent`.

import { useTranslation } from 'react-i18next'
import type { PortalView } from '@/stores/portal'
import { MarkdownPreview } from './markdown-preview'

interface FilePreviewViewProps {
  view: Extract<PortalView, { type: 'filePreview' }>
}

/** Last path segment, no directory components. Falls back to the full path. */
function basename(path: string): string {
  if (!path) return ''
  const idx = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'))
  return idx >= 0 ? path.slice(idx + 1) : path
}

/** Case-insensitive `.md` / `.markdown` detection. */
function isMarkdownPath(path: string): boolean {
  const lower = path.toLowerCase()
  return lower.endsWith('.md') || lower.endsWith('.markdown')
}

export function FilePreviewView({ view }: FilePreviewViewProps) {
  const { t } = useTranslation()
  const { path, content } = view
  const name = basename(path)
  const isMarkdown = isMarkdownPath(path)
  const isLoading = content === undefined

  return (
    <div className="flex h-full flex-col">
      {/* Sub-header: full path + filename highlight. Mirrors artifact-view's
          actions row visually so the panel chrome reads as one consistent
          family. */}
      <div className="flex items-center justify-between gap-2 border-b px-3 py-1.5">
        <div className="flex min-w-0 items-center gap-2 text-xs text-muted-foreground">
          <span className="truncate font-mono" title={path}>
            {path}
          </span>
        </div>
        {isMarkdown ? (
          <span className="shrink-0 rounded border px-1.5 py-0.5 font-mono text-[10px] uppercase tracking-wide text-muted-foreground">
            MD
          </span>
        ) : null}
      </div>

      {/* Body */}
      <div className="min-h-0 flex-1 overflow-auto">
        {isLoading ? (
          <div className="flex h-full items-center justify-center text-sm text-muted-foreground">
            <span className="inline-block size-2 animate-pulse rounded-full bg-status-warning" />
            <span className="ms-2">{t('portal.filePreview.loading')}</span>
          </div>
        ) : isMarkdown ? (
          <MarkdownPreview content={content ?? ''} className="p-4" />
        ) : (
          <pre className="h-full overflow-auto bg-muted/40 p-4 font-mono text-xs leading-relaxed">
            <code>{content ?? ''}</code>
          </pre>
        )}
        {/* Visually hidden filename for screen readers — the title is also
            rendered in the shared PortalHeader. */}
        <span className="sr-only">{name}</span>
      </div>
    </div>
  )
}
