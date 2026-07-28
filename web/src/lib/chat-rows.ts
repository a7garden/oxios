// chat-rows — pure row model for the virtualized chat list (LobeHub borrow).
//
// Flattens the conversation into a heterogeneous row array consumed by virtua's
// <VList>: an optional collapse bar (older messages folded past a threshold),
// the message rows, and any active intervention cards (interview / tool
// approval / path access). `index` on a message row is its position in the
// FULL messages array — used to derive assistantIndex and minimap jumps.

import type { ChatMessage } from '@/types'

/** One renderable row in the virtualized chat list. */
export type ChatRow =
  | { kind: 'empty' }
  | { kind: 'collapse-bar'; count: number }
  | { kind: 'message'; message: ChatMessage; index: number }
  | { kind: 'interview' }
  | { kind: 'tool-approval' }
  | { kind: 'path-access' }

export interface BuildChatRowsOptions {
  messages: ChatMessage[]
  /** Whether the collapse group is expanded (show all messages). */
  expanded: boolean
  /** Message count above which older messages collapse. */
  collapseThreshold: number
  /** Number of recent messages kept visible when collapsed. */
  visibleTail: number
  hasInterview: boolean
  hasToolApproval: boolean
  hasPathAccess: boolean
}

export function buildChatRows(opts: BuildChatRowsOptions): ChatRow[] {
  const { messages, expanded, collapseThreshold, visibleTail } = opts
  const hasCard = opts.hasInterview || opts.hasToolApproval || opts.hasPathAccess

  if (messages.length === 0 && !hasCard) return [{ kind: 'empty' }]

  const rows: ChatRow[] = []
  const collapseCount = messages.length > collapseThreshold ? messages.length - visibleTail : 0

  if (collapseCount > 0) {
    rows.push({ kind: 'collapse-bar', count: collapseCount })
    const start = expanded ? 0 : collapseCount
    for (let i = start; i < messages.length; i++) {
      rows.push({ kind: 'message', message: messages[i]!, index: i })
    }
  } else {
    for (let i = 0; i < messages.length; i++) {
      rows.push({ kind: 'message', message: messages[i]!, index: i })
    }
  }

  if (opts.hasInterview) rows.push({ kind: 'interview' })
  if (opts.hasToolApproval) rows.push({ kind: 'tool-approval' })
  if (opts.hasPathAccess) rows.push({ kind: 'path-access' })
  return rows
}
