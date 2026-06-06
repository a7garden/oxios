import {
  Search, ClipboardList, Zap, XCircle, FileText, Camera,
  ExternalLink, LoaderCircle, ScrollText,
} from 'lucide-react'
import type { ToolCallContext, VisitReason } from '@/types'

/**
 * Inline badge showing what a browsing tool is doing.
 *
 * Renders differently per context kind:
 * - page_visit: shows URL (loading) → title + status + duration (loaded) → error
 * - web_search: shows query + engine
 * - data_extraction: shows target + count
 * - session_action: shows action
 * - script_step: shows step progress (current/total) + progress bar
 */
export function BrowseContextBadge({ context }: { context: ToolCallContext }) {
  switch (context.kind) {
    case 'page_visit':
      return <PageVisitBadge context={context} />
    case 'web_search':
      return (
        <span className="inline-flex items-center gap-1 text-2xs text-info truncate max-w-[50ch]">
          <Search className="h-3 w-3 shrink-0" />
          {context.query ? context.query : ''}
          {context.engine ? <span className="text-muted-foreground">({context.engine})</span> : null}
        </span>
      )
    case 'data_extraction':
      return (
        <span className="inline-flex items-center gap-1 text-2xs text-warning truncate max-w-[50ch]">
          <ClipboardList className="h-3 w-3 shrink-0" />
          {context.target}
          {context.result_count != null ? (
            <span className="text-muted-foreground">({context.result_count} items)</span>
          ) : null}
        </span>
      )
    case 'session_action':
      return (
        <span className="inline-flex items-center gap-1 text-2xs text-purple-500 dark:text-purple-400 truncate max-w-[40ch]">
          <Zap className="h-3 w-3 shrink-0" />
          {context.action}
        </span>
      )
    case 'script_step':
      return <ScriptStepBadge context={context} />
  }
}

function ScriptStepBadge({
  context,
}: {
  context: Extract<ToolCallContext, { kind: 'script_step' }>
}) {
  const pct = context.total > 0 ? Math.round((context.current / context.total) * 100) : 0

  return (
    <span className="inline-flex items-center gap-1.5 text-2xs text-muted-foreground">
      <ScrollText className="h-3 w-3 shrink-0" />
      <span className="flex items-center gap-1">
        <span
          className="inline-block w-10 h-1.5 rounded-full bg-muted overflow-hidden"
          role="progressbar"
          aria-valuenow={pct}
          aria-valuemin={0}
          aria-valuemax={100}
        >
          <span
            className="block h-full rounded-full bg-info transition-all duration-300"
            style={{ width: `${pct}%` }}
          />
        </span>
        <span>
          {context.current}/{context.total}
        </span>
      </span>
      <span className="truncate max-w-[24ch]">{context.step}</span>
    </span>
  )
}

function PageVisitBadge({
  context,
}: {
  context: Extract<ToolCallContext, { kind: 'page_visit' }>
}) {
  // Navigation failed
  if (context.navigation_error) {
    return (
      <span className="inline-flex items-center gap-1 text-2xs text-error truncate max-w-[50ch]">
        <XCircle className="h-3 w-3 shrink-0" />
        <span className="truncate font-mono">{shortenUrl(context.url)}</span>
        <span className="text-muted-foreground truncate max-w-[24ch]">— {context.navigation_error}</span>
      </span>
    )
  }

  // If we have a page title, the page has loaded
  if (context.page_title) {
    return (
      <span className="inline-flex items-center gap-1 text-2xs text-success truncate max-w-[50ch]">
        <FileText className="h-3 w-3 shrink-0" />
        <VisitReasonChip reason={context.reason} />
        <span className="truncate">{context.page_title}</span>
        {context.page_status ? (
          <span className={statusColor(context.page_status)}>[{context.page_status}]</span>
        ) : null}
        {context.page_duration_ms != null ? (
          <span className="text-muted-foreground">{formatDuration(context.page_duration_ms)}</span>
        ) : null}
        {context.screenshot ? <Camera className="h-3 w-3 text-muted-foreground" /> : null}
      </span>
    )
  }

  // Still loading — show URL with visit reason
  return (
    <span className="inline-flex items-center gap-1 text-2xs text-muted-foreground truncate max-w-[50ch]">
      <LoaderCircle className="h-3 w-3 shrink-0 animate-spin" />
      <VisitReasonChip reason={context.reason} />
      <span className="truncate font-mono">{shortenUrl(context.url)}</span>
    </span>
  )
}

/** Small chip indicating why the page is being visited. */
function VisitReasonChip({ reason }: { reason?: VisitReason }) {
  if (!reason) return null
  if (reason === 'direct_navigation') return null
  if ('search_result' in reason) {
    const pos = reason.search_result.position
    return (
      <span
        className="text-info shrink-0"
        title={`Search result #${pos}`}
      >
        #{pos}
      </span>
    )
  }
  if ('link_followed' in reason) {
    return (
      <span
        className="text-muted-foreground shrink-0"
        title={`Followed link from ${reason.link_followed.from_url}`}
      >
          <ExternalLink className="h-3 w-3" />
      </span>
    )
  }
  return null
}

function statusColor(status: number): string {
  if (status >= 200 && status < 300) return 'text-success'
  if (status >= 300 && status < 400) return 'text-warning'
  if (status >= 400) return 'text-error'
  return 'text-muted-foreground'
}

function shortenUrl(url: string): string {
  try {
    const u = new URL(url)
    const path = u.pathname === '/' ? '' : u.pathname
    return `${u.host}${path}`
  } catch {
    return url.length > 40 ? url.slice(0, 40) + '…' : url
  }
}

function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`
  return `${(ms / 1000).toFixed(1)}s`
}
