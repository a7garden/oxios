import { Bot, ExternalLink, Power, Square, X } from 'lucide-react'
import { useEffect, useRef } from 'react'
import { useTranslation } from 'react-i18next'
import { statusDot } from '@/components/shared/status-palette'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import type { A2AAgentCard, A2AMessage, TopologyNode } from '@/types/a2a'

interface Props {
  /** The currently selected node (from topology). */
  node: TopologyNode | null
  /** Open/closed state. */
  open: boolean
  /** Close handler. */
  onClose: () => void
  /** Full agent card detail (for capabilities/skills list). */
  agentCard?: A2AAgentCard | null
  /** Recent messages involving this agent. */
  recentMessages?: A2AMessage[]
  /** Loading state for the messages. */
  isMessagesLoading?: boolean
  /** Stop-agent handler (placeholder — wired in a follow-up). */
  onStopAgent?: (id: string) => void
  /** View-trace handler (placeholder — wired in a follow-up). */
  onViewTrace?: (id: string) => void
}

/**
 * Slide-in inspector for a single A2A agent.
 *
 * Slides from the right when `open` is true. Closes on backdrop click,
 * Esc key, or close button. Renders the agent's capabilities, skills,
 * status, and the last 5 messages.
 */
export function AgentInspector({
  node,
  open,
  onClose,
  agentCard,
  recentMessages = [],
  isMessagesLoading,
  onStopAgent,
  onViewTrace,
}: Props) {
  const { t } = useTranslation()
  const panelRef = useRef<HTMLDivElement>(null)
  const closeButtonRef = useRef<HTMLButtonElement>(null)

  // Esc to close
  useEffect(() => {
    if (!open) return
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation()
        onClose()
      }
    }
    document.addEventListener('keydown', onKey)
    return () => document.removeEventListener('keydown', onKey)
  }, [open, onClose])

  // Focus trap: apply `inert` to the rest of the page so Tab cycles
  // inside the dialog instead of leaking out. The inspector is a
  // fixed-position overlay (sibling of <main>), so toggling `inert`
  // on <main> is sufficient — React 19's types support it natively.
  useEffect(() => {
    const main = document.querySelector('main')
    if (!main) return
    if (open) {
      // Save the previous value so we can restore on close.
      const prev = main.getAttribute('inert')
      main.setAttribute('inert', '')
      return () => {
        if (prev === null) {
          main.removeAttribute('inert')
        } else {
          main.setAttribute('inert', prev)
        }
      }
    }
    return undefined
  }, [open])

  // Focus close button when opened.
  // Skip if focus is already inside the panel (e.g. user clicked
  // a tab in the tab switcher) to avoid stealing focus mid-task.
  useEffect(() => {
    if (!open) return
    const id = requestAnimationFrame(() => {
      const panel = panelRef.current
      if (!panel) return
      const alreadyInside = panel.contains(document.activeElement)
      if (!alreadyInside) {
        closeButtonRef.current?.focus()
      }
    })
    return () => cancelAnimationFrame(id)
  }, [open])

  if (!node) return null

  const dot = statusDot(node.status)
  const capList = agentCard?.capabilities ?? node.capabilities ?? []
  const skillList = agentCard?.skills ?? node.skills ?? []

  return (
    <div
      aria-hidden={!open}
      className={cn(
        'fixed inset-0 z-40 transition-opacity',
        open ? 'opacity-100 pointer-events-auto' : 'opacity-0 pointer-events-none',
      )}
    >
      {/* Backdrop */}
      <div className="absolute inset-0 bg-black/30" onClick={onClose} aria-hidden="true" />
      {/* Panel */}
      <aside
        ref={panelRef}
        role="dialog"
        aria-modal="true"
        aria-label={t('a2a.inspectorTitle', { name: node.label })}
        data-testid="a2a-agent-inspector"
        className={cn(
          'absolute right-0 top-0 h-full w-full max-w-md bg-card border-l shadow-xl',
          'transition-transform duration-200 ease-out',
          open ? 'translate-x-0' : 'translate-x-full',
        )}
      >
        <div className="flex h-full flex-col">
          {/* Header */}
          <header className="flex items-start justify-between border-b p-4">
            <div className="flex items-start gap-3">
              <div className="rounded-md bg-muted p-2">
                <Bot className="h-5 w-5" aria-hidden="true" />
              </div>
              <div>
                <h2 className="text-base font-semibold">{node.label}</h2>
                <div className="flex items-center gap-1.5 mt-0.5">
                  <span className={cn('h-2 w-2 rounded-full', dot)} aria-hidden="true" />
                  <span className="text-xs capitalize text-muted-foreground">{node.status}</span>
                </div>
              </div>
            </div>
            <Button
              ref={closeButtonRef}
              variant="ghost"
              size="icon"
              onClick={onClose}
              aria-label={t('a2a.inspectorClose')}
            >
              <X className="h-4 w-4" />
            </Button>
          </header>

          {/* Body */}
          <div className="flex-1 overflow-y-auto p-4 space-y-5">
            <section>
              <h3 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                {t('a2a.inspectorCapabilities')}
              </h3>
              {capList.length === 0 ? (
                <p className="text-sm text-muted-foreground mt-2">{t('a2a.inspectorNoCaps')}</p>
              ) : (
                <div className="flex flex-wrap gap-1.5 mt-2">
                  {capList.map((c) => (
                    <Badge key={c} variant="outline" className="text-xs">
                      {c}
                    </Badge>
                  ))}
                </div>
              )}
            </section>

            <section>
              <h3 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                {t('a2a.inspectorSkills')}
              </h3>
              {skillList.length === 0 ? (
                <p className="text-sm text-muted-foreground mt-2">{t('a2a.inspectorNoSkills')}</p>
              ) : (
                <div className="flex flex-wrap gap-1.5 mt-2">
                  {skillList.map((s) => (
                    <Badge key={s} variant="secondary" className="text-xs">
                      {s}
                    </Badge>
                  ))}
                </div>
              )}
            </section>

            <section>
              <h3 className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
                {t('a2a.inspectorLastMessages')}
              </h3>
              {isMessagesLoading ? (
                <p className="text-sm text-muted-foreground mt-2">{t('a2a.inspectorLoading')}</p>
              ) : recentMessages.length === 0 ? (
                <p className="text-sm text-muted-foreground mt-2">{t('a2a.inspectorNoMessages')}</p>
              ) : (
                <ul className="mt-2 space-y-2" data-testid="a2a-inspector-messages">
                  {recentMessages.slice(0, 5).map((m) => (
                    <li
                      key={m.request_id}
                      className="rounded-md border bg-background p-2 text-xs space-y-1"
                    >
                      <div className="flex items-center justify-between">
                        <span className="font-mono text-[10px] text-muted-foreground">
                          {new Date(m.timestamp).toLocaleTimeString()}
                        </span>
                        <Badge variant="outline" className="text-[10px]">
                          {m.message_type}
                        </Badge>
                      </div>
                      <div>
                        <span className="font-medium">{m.from_agent}</span>
                        <span className="text-muted-foreground mx-1">→</span>
                        <span className="font-medium">{m.to_agent}</span>
                      </div>
                      {m.payload_summary && (
                        <p className="text-muted-foreground truncate">{m.payload_summary}</p>
                      )}
                    </li>
                  ))}
                </ul>
              )}
            </section>
          </div>

          {/* Footer actions */}
          <footer className="border-t p-4 flex items-center gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={() => onViewTrace?.(node.id)}
              className="flex-1"
            >
              <ExternalLink className="h-3.5 w-3.5" />
              {t('a2a.inspectorViewTrace')}
            </Button>
            <Button
              variant="destructive"
              size="sm"
              onClick={() => onStopAgent?.(node.id)}
              className="flex-1"
            >
              <Square className="h-3.5 w-3.5" />
              {t('a2a.inspectorStopAgent')}
            </Button>
          </footer>

          <div className="px-4 pb-4 text-[10px] text-muted-foreground flex items-center gap-1">
            <Power className="h-3 w-3" aria-hidden="true" />
            {node.last_seen
              ? `${t('a2a.inspectorLastSeen')}: ${new Date(node.last_seen).toLocaleString()}`
              : t('a2a.neverSeen')}
          </div>
        </div>
      </aside>
    </div>
  )
}
