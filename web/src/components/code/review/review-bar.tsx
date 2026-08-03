// review-bar — collapsible bar that surfaces "N files changed" above
// the composer whenever the agent has produced pending edits.
//
// The bar stays out of the layout entirely when there are no pending
// changes; expanding it mounts the ReviewDiff list so we don't pay for
// parsing diffs that the user never opens.

import { CheckCheck, ChevronDown, ChevronUp, FileCode2 } from 'lucide-react'
import { useState } from 'react'
import { useCodeSessionStore } from '@/stores/code/code-session'
import { cn } from '@/lib/utils'
import { Button } from '@/components/ui/button'
import { ReviewDiff } from './review-diff'

export interface ReviewBarProps {
  /** Optional className for the outer wrapper. */
  className?: string
}

/**
 * ReviewBar — thin accordion rendered above the agent composer.
 * Shows the count of pending file changes and toggles the diff viewer.
 */
export function ReviewBar({ className }: ReviewBarProps) {
  const pendingChanges = useCodeSessionStore((s) => s.pendingChanges)
  const count = pendingChanges.length

  // Local UI state — the user has to opt in to seeing the diffs, so
  // we don't reset this on store changes.
  const [expanded, setExpanded] = useState(false)

  if (count === 0) return null

  return (
    <div
      className={cn(
        'border-t border-line bg-surface-sunken text-foreground',
        className,
      )}
    >
      <div className="flex items-center gap-2 px-3 py-1.5">
        <Button
          type="button"
          variant="ghost"
          size="sm"
          onClick={() => setExpanded((v) => !v)}
          className="h-7 px-2 text-xs gap-1.5"
          aria-expanded={expanded}
          aria-controls="review-bar-content"
        >
          <FileCode2 className="size-3.5 text-primary" />
          <span className="font-medium">
            {count} {count === 1 ? 'file changed' : 'files changed'}
          </span>
          {expanded ? (
            <ChevronUp className="size-3.5 text-muted-foreground" />
          ) : (
            <ChevronDown className="size-3.5 text-muted-foreground" />
          )}
        </Button>
        <span className="ml-auto flex items-center gap-1 text-[10px] text-muted-foreground">
          <CheckCheck className="size-3" />
          <span>Review before continuing</span>
        </span>
      </div>
      {expanded ? (
        <div
          id="review-bar-content"
          className="px-3 pb-3 max-h-96 overflow-auto"
        >
          <ReviewDiff />
        </div>
      ) : null}
    </div>
  )
}
