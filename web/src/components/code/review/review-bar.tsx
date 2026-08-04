import { Check, ChevronDown, ChevronUp, FileEdit, FileMinus, FilePlus, X } from 'lucide-react'
import { useState } from 'react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Separator } from '@/components/ui/separator'
import { codeApi } from '@/lib/code-api'
import { useCodeSessionStore } from '@/stores/code/code-session'
import type { FileChange } from '@/types/code'

/**
 * Change review bar + diff viewer.
 * Shows "N files changed" above the agent input. Expands to show
 * per-file diffs with accept/reject buttons.
 */
export function ReviewBar() {
  const { pendingChanges, session } = useCodeSessionStore()
  const [expanded, setExpanded] = useState(false)

  if (pendingChanges.length === 0) return null

  const pendingCount = pendingChanges.filter((c) => !c.accepted).length

  async function acceptAll() {
    if (!session) return
    await codeApi.acceptAllChanges(session.id)
  }

  async function rejectAll() {
    if (!session) return
    await codeApi.rejectAllChanges(session.id)
  }

  return (
    <div className="border-t border-border">
      <button
        type="button"
        className="flex w-full items-center gap-2 px-3 py-2 text-sm hover:bg-surface-sunken transition-colors"
        onClick={() => setExpanded((v) => !v)}
      >
        {expanded ? (
          <ChevronDown className="h-4 w-4 text-muted-foreground" />
        ) : (
          <ChevronUp className="h-4 w-4 text-muted-foreground" />
        )}
        <span className="font-medium">
          {pendingCount} file{pendingCount !== 1 ? 's' : ''} changed
        </span>
        <div className="flex-1" />
        <Button
          variant="ghost"
          size="sm"
          className="h-7 text-xs"
          onClick={(e) => {
            e.stopPropagation()
            acceptAll()
          }}
        >
          <Check className="mr-1 h-3 w-3" /> Accept All
        </Button>
        <Button
          variant="ghost"
          size="sm"
          className="h-7 text-xs text-red-500"
          onClick={(e) => {
            e.stopPropagation()
            rejectAll()
          }}
        >
          <X className="mr-1 h-3 w-3" /> Reject All
        </Button>
      </button>

      {expanded && (
        <ScrollArea className="max-h-96 border-t border-border">
          <div className="p-2 space-y-2">
            {pendingChanges.map((change, i) => (
              <DiffCard key={`${change.path}-${i}`} change={change} />
            ))}
          </div>
        </ScrollArea>
      )}
    </div>
  )
}

function DiffCard({ change }: { change: FileChange }) {
  const { session } = useCodeSessionStore()
  const [accepted, setAccepted] = useState(change.accepted)

  const fileName = change.path.split('/').pop() || change.path
  const actionConfig = {
    create: { icon: FilePlus, label: 'Created', variant: 'success' as const },
    modify: { icon: FileEdit, label: 'Modified', variant: 'warning' as const },
    delete: { icon: FileMinus, label: 'Deleted', variant: 'destructive' as const },
  }
  const config = actionConfig[change.action]
  const Icon = config.icon

  async function accept() {
    if (!session) return
    setAccepted(true)
  }

  async function reject() {
    if (!session) return
    setAccepted(true)
  }

  if (accepted) return null

  return (
    <div className="rounded-lg border border-border bg-surface p-3">
      <div className="flex items-center gap-2 mb-2">
        <Icon className="h-4 w-4 text-muted-foreground" />
        <span className="font-mono text-sm truncate flex-1">{fileName}</span>
        <Badge variant={config.variant}>{config.label}</Badge>
      </div>

      {change.diff && (
        <pre className="text-xs font-mono bg-surface-sunken rounded-md p-2 overflow-x-auto max-h-48">
          {change.diff.split('\n').map((line, i) => {
            let className = 'text-muted-foreground'
            if (line.startsWith('+') && !line.startsWith('+++')) {
              className = 'text-success'
            } else if (line.startsWith('-') && !line.startsWith('---')) {
              className = 'text-error'
            }
            return (
              <div key={i} className={className}>
                {line || ' '}
              </div>
            )
          })}
        </pre>
      )}

      <Separator className="my-2" />

      <div className="flex justify-end gap-2">
        <Button variant="outline" size="sm" className="h-7 text-xs" onClick={reject}>
          <X className="mr-1 h-3 w-3" /> Reject
        </Button>
        <Button variant="default" size="sm" className="h-7 text-xs" onClick={accept}>
          <Check className="mr-1 h-3 w-3" /> Accept
        </Button>
      </div>
    </div>
  )
}
