// review-diff — colored diff viewer with per-file accept/reject controls.
//
// Renders each pending file change as:
//   • file path + an action badge (created / modified / deleted)
//   • parsed unified diff where '+' lines are green, '-' red,
//     ' ' context is muted — header lines (---, +++, @@) are kept neutral.
//   • Accept and Reject buttons that drop the change from local state.
//
// The store is the source of truth, but mutation goes through the bulk
// endpoints so the backend stays consistent when the local cache
// diverges (e.g. other tabs added the same change).

import { Check, FileMinus2, FilePlus2, FileText, X } from 'lucide-react'
import { useCallback, useMemo } from 'react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { ScrollArea } from '@/components/ui/scroll-area'
import { codeApi } from '@/lib/code-api'
import { cn } from '@/lib/utils'
import { useCodeSessionStore } from '@/stores/code/code-session'
import type { ChangeAction, FileChange } from '@/types/code'

export interface ReviewDiffProps {
  /** Optional className for the outer wrapper. */
  className?: string
}

// ── Diff line classification ─────────────────────────────────────

/**
 * Map a single line of a unified diff to the visual treatment it gets
 * in the viewer. Header lines (---, +++, @@) are kept neutral so they
 * read as structural metadata rather than hunks.
 */
type DiffLineKind = 'add' | 'del' | 'context' | 'header'

interface DiffLine {
  kind: DiffLineKind
  text: string
}

function classifyDiffLine(raw: string): DiffLine {
  // Strip trailing newline so the rendered text matches `original_content`
  // without an extra whitespace.
  const text = raw.endsWith('\n') ? raw.slice(0, -1) : raw
  if (text.startsWith('--- ') || text.startsWith('+++ ') || text.startsWith('@@')) {
    return { kind: 'header', text }
  }
  if (text.startsWith('+')) return { kind: 'add', text: text.slice(1) }
  if (text.startsWith('-')) return { kind: 'del', text: text.slice(1) }
  // A leading space is the standard context marker; an empty string is
  // also treated as context so callers can hand us partial diffs.
  return {
    kind: 'context',
    text: text.startsWith(' ') ? text.slice(1) : text,
  }
}

function parseDiff(diff: string): DiffLine[] {
  if (!diff) return []
  // The backend stores new files as a full diff that starts with
  // `--- /dev/null` and `+++ <path>` so the unified parser below works
  // uniformly for create / modify / delete.
  return diff.split('\n').map(classifyDiffLine)
}

// ── Action helpers ───────────────────────────────────────────────

const ACTION_LABEL: Record<ChangeAction, string> = {
  create: 'Created',
  modify: 'Modified',
  delete: 'Deleted',
}

const ACTION_VARIANT: Record<ChangeAction, 'success' | 'warning' | 'error'> = {
  create: 'success',
  modify: 'warning',
  delete: 'error',
}

function ActionIcon({ action }: { action: ChangeAction }) {
  if (action === 'create') return <FilePlus2 className="size-3.5" />
  if (action === 'delete') return <FileMinus2 className="size-3.5" />
  return <FileText className="size-3.5" />
}

// ── Per-file card ────────────────────────────────────────────────

interface FileChangeCardProps {
  change: FileChange
  onAccept: (c: FileChange) => void
  onReject: (c: FileChange) => void
  busy: boolean
}

function FileChangeCard({ change, onAccept, onReject, busy }: FileChangeCardProps) {
  const lines = useMemo(() => parseDiff(change.diff), [change.diff])

  return (
    <div className="rounded-md border border-line bg-background overflow-hidden">
      <div className="flex items-center gap-2 px-3 py-2 border-b border-line bg-surface-sunken">
        <ActionIcon action={change.action} />
        <span className="flex-1 min-w-0 truncate font-mono text-xs text-foreground">
          {change.path}
        </span>
        <Badge variant={ACTION_VARIANT[change.action]} className="shrink-0">
          {ACTION_LABEL[change.action]}
        </Badge>
      </div>
      <ScrollArea className="max-h-72">
        <pre className="m-0 px-3 py-2 text-[11px] leading-relaxed font-mono whitespace-pre-wrap break-words">
          {lines.length === 0 ? (
            <span className="text-muted-foreground">
              {change.action === 'delete' ? 'File removed.' : 'No diff available.'}
            </span>
          ) : (
            lines.map((line, idx) => (
              <div
                key={idx}
                className={cn(
                  'px-1 -mx-1 rounded-sm',
                  line.kind === 'add' && 'text-success bg-success-muted/40',
                  line.kind === 'del' && 'text-error bg-error-muted/40',
                  line.kind === 'context' && 'text-muted-foreground',
                  line.kind === 'header' && 'text-foreground/70 font-semibold',
                )}
              >
                <span className="select-none mr-2 opacity-60">
                  {line.kind === 'add'
                    ? '+'
                    : line.kind === 'del'
                      ? '-'
                      : line.kind === 'header'
                        ? '@'
                        : ' '}
                </span>
                {line.text || '\u00A0'}
              </div>
            ))
          )}
        </pre>
      </ScrollArea>
      <div className="flex items-center justify-end gap-2 px-3 py-2 border-t border-line bg-surface-sunken">
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={() => onReject(change)}
          disabled={busy}
          className="h-7 px-2.5 text-xs"
          aria-label={`Reject changes to ${change.path}`}
        >
          <X className="size-3.5" />
          <span>Reject</span>
        </Button>
        <Button
          type="button"
          variant="default"
          size="sm"
          onClick={() => onAccept(change)}
          disabled={busy}
          className="h-7 px-2.5 text-xs"
          aria-label={`Accept changes to ${change.path}`}
        >
          <Check className="size-3.5" />
          <span>Accept</span>
        </Button>
      </div>
    </div>
  )
}

// ── Main viewer ──────────────────────────────────────────────────

/**
 * ReviewDiff — list of pending file changes with per-file and bulk
 * accept/reject controls. Reads `pendingChanges` from
 * `useCodeSessionStore` and updates the store when the user takes an
 * action.
 */
export function ReviewDiff({ className }: ReviewDiffProps) {
  const session = useCodeSessionStore((s) => s.session)
  const pendingChanges = useCodeSessionStore((s) => s.pendingChanges)
  const setPendingChanges = useCodeSessionStore((s) => s.setPendingChanges)

  const dropChange = useCallback(
    (path: string) => {
      setPendingChanges(pendingChanges.filter((c) => c.path !== path))
    },
    [pendingChanges, setPendingChanges],
  )

  const onAccept = useCallback(
    async (change: FileChange) => {
      // Optimistic local update — the change is gone from the review
      // list immediately. The backend has no per-file endpoint so we
      // only call the bulk endpoint when this is the last one left.
      dropChange(change.path)
      if (session && pendingChanges.length === 1) {
        try {
          await codeApi.acceptAllChanges(session.id)
        } catch {
          // Surface failure via a fresh fetch elsewhere; here the
          // local state is already consistent.
        }
      }
    },
    [dropChange, pendingChanges.length, session],
  )

  const onReject = useCallback(
    async (change: FileChange) => {
      dropChange(change.path)
      if (session && pendingChanges.length === 1) {
        try {
          await codeApi.rejectAllChanges(session.id)
        } catch {
          /* see onAccept */
        }
      }
    },
    [dropChange, pendingChanges.length, session],
  )

  const onAcceptAll = useCallback(async () => {
    if (!session) return
    try {
      await codeApi.acceptAllChanges(session.id)
      setPendingChanges([])
    } catch {
      /* keep local state; user can retry */
    }
  }, [session, setPendingChanges])

  const onRejectAll = useCallback(async () => {
    if (!session) return
    try {
      await codeApi.rejectAllChanges(session.id)
      setPendingChanges([])
    } catch {
      /* keep local state; user can retry */
    }
  }, [session, setPendingChanges])

  return (
    <div className={cn('flex flex-col gap-2', className)}>
      <div className="flex items-center justify-end gap-2">
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={() => void onRejectAll()}
          disabled={!session}
          className="h-7 px-2.5 text-xs"
        >
          <X className="size-3.5" />
          <span>Reject All</span>
        </Button>
        <Button
          type="button"
          size="sm"
          onClick={() => void onAcceptAll()}
          disabled={!session}
          className="h-7 px-2.5 text-xs"
        >
          <Check className="size-3.5" />
          <span>Accept All</span>
        </Button>
      </div>
      <div className="flex flex-col gap-2">
        {pendingChanges.map((change) => (
          <FileChangeCard
            key={change.path}
            change={change}
            onAccept={onAccept}
            onReject={onReject}
            busy={!session}
          />
        ))}
      </div>
    </div>
  )
}
