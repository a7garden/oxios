import type { ToolCallContext, VisitReason, ScreenshotMeta } from '@/types'

/**
 * Expanded detail view for a browsing tool's semantic context.
 *
 * Shown inside `ActivityDetail` when a tool_call activity is expanded.
 * Renders a structured summary of what the browsing tool did — URL,
 * page title, status, bytes extracted, duration — depending on the
 * context kind.
 */
export function BrowseContextDetail({ context }: { context: ToolCallContext }) {
  switch (context.kind) {
    case 'page_visit':
      return <PageVisitDetail context={context} />
    case 'web_search':
      return (
        <div className="space-y-1">
          <DetailRow label="Query" value={context.query} />
          {context.engine ? <DetailRow label="Engine" value={context.engine} /> : null}
        </div>
      )
    case 'data_extraction':
      return (
        <div className="space-y-1">
          <DetailRow label="Target" value={context.target} />
          {context.url ? <DetailRow label="URL" value={context.url} mono /> : null}
          {context.result_count != null ? (
            <DetailRow label="Extracted" value={`${context.result_count} items`} />
          ) : null}
          {context.page_status != null ? (
            <DetailRow label="Status" value={`${context.page_status}`} />
          ) : null}
          {context.page_duration_ms != null ? (
            <DetailRow label="Duration" value={formatDuration(context.page_duration_ms)} />
          ) : null}
        </div>
      )
    case 'session_action':
      return (
        <div className="space-y-1">
          <DetailRow label="Action" value={context.action} />
          {context.url ? <DetailRow label="URL" value={context.url} mono /> : null}
        </div>
      )
    case 'script_step':
      return (
        <div className="space-y-1">
          <DetailRow label="Step" value={`${context.current} / ${context.total}`} />
          <DetailRow label="Description" value={context.step} />
          {context.total > 0 && (
            <div className="mt-1.5">
              <div className="w-full h-1.5 rounded-full bg-muted overflow-hidden">
                <div
                  className="h-full rounded-full bg-info transition-all duration-300"
                  style={{ width: `${Math.round((context.current / context.total) * 100)}%` }}
                />
              </div>
            </div>
          )}
        </div>
      )
  }
}

function PageVisitDetail({
  context,
}: {
  context: Extract<ToolCallContext, { kind: 'page_visit' }>
}) {
  return (
    <div className="space-y-1">
      <DetailRow label="URL" value={context.url} mono />
      {context.reason ? <VisitReasonDetail reason={context.reason} /> : null}
      {context.navigation_error ? (
        <DetailRow label="Error" value={context.navigation_error} />
      ) : null}
      {context.page_title ? <DetailRow label="Title" value={context.page_title} /> : null}
      {context.page_status != null ? (
        <DetailRow label="Status" value={`${context.page_status}`} />
      ) : null}
      {context.page_bytes != null ? (
        <DetailRow label="Size" value={formatBytes(context.page_bytes)} />
      ) : null}
      {context.page_duration_ms != null ? (
        <DetailRow label="Duration" value={formatDuration(context.page_duration_ms)} />
      ) : null}
      {context.screenshot ? <ScreenshotDetail meta={context.screenshot} /> : null}
    </div>
  )
}

function VisitReasonDetail({ reason }: { reason: VisitReason }) {
  if (reason === 'direct_navigation') {
    return <DetailRow label="Reason" value="Direct navigation" />
  }
  if ('search_result' in reason) {
    return <DetailRow label="Reason" value={`Search result #${reason.search_result.position}`} />
  }
  if ('link_followed' in reason) {
    return <DetailRow label="Reason" value="Followed link" />
  }
  return null
}

function ScreenshotDetail({ meta }: { meta: ScreenshotMeta }) {
  return (
    <div className="space-y-1">
      <p className="text-2xs font-medium text-muted-foreground mb-1 uppercase tracking-wider">
        Screenshot
      </p>
      <div className="rounded border bg-muted/50 px-2 py-1.5 space-y-0.5">
        <DetailRow label="Size" value={formatBytes(meta.bytes)} />
        <DetailRow label="Width" value={`${meta.width}px`} />
        <DetailRow label="Duration" value={formatDuration(meta.duration_ms)} />
      </div>
    </div>
  )
}

function DetailRow({
  label,
  value,
  mono = false,
}: {
  label: string
  value: string
  mono?: boolean
}) {
  return (
    <div className="flex items-baseline gap-2 text-xs">
      <span className="text-muted-foreground font-medium min-w-[60px] shrink-0">{label}</span>
      <span className={mono ? 'font-mono break-all' : 'break-words'}>{value}</span>
    </div>
  )
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`
}

function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`
  return `${(ms / 1000).toFixed(1)}s`
}
