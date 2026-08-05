// FanOutButton — composer affordance for the `worktree-fanout` capability.
//
// RFC-044 Phase 3: when the active persona exposes the `worktree-fanout`
// capability, this button appears in the chat composer toolbar. Clicking
// it opens a dialog with a prompt input + count selector (2-8). On submit,
// the backend spawns N parallel worktree agents via POST /api/worktree/fanout.
//
// Spawned agents are tracked in the fanout store and rendered inline in the
// chat transcript as AgentFanoutCard grids. We also surface a tiny "in
// progress" badge while the request is pending.

import { GitFork, Loader2 } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Label } from '@/components/ui/label'
import { Select as SimpleSelect } from '@/components/ui/select'
import { Textarea } from '@/components/ui/textarea'
import { api } from '@/lib/api-client'
import { useChatStore } from '@/stores/chat'
import { useFanoutStore } from '@/stores/fanout'

/** Allowed fan-out sizes. Backend supports 2-8 parallel agents. */
const COUNT_OPTIONS = [2, 3, 4, 5, 6, 7, 8] as const

/** Response shape from POST /api/worktree/fanout. */
interface FanoutResponse {
  group_id?: string
  agents?: Array<{
    agent_id: string
    name?: string
    worktree_path?: string
  }>
}

interface FanOutButtonProps {
  /** Optional project path override. Falls back to the active session's
   *  project, then to an empty string (server decides). */
  projectPath?: string
  /** Show the Fan Out label next to the icon. Defaults to icon-only. */
  showLabel?: boolean
  /** Optional className passthrough. */
  className?: string
}

/**
 * FanOutButton
 *
 * Pure-UI affordance: hidden unless the active persona carries
 * `worktree-fanout`. Clicking opens a dialog; submitting calls the API and
 * tracks the spawned agents in the fanout store.
 */
export function FanOutButton({ projectPath, showLabel, className }: FanOutButtonProps) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const [prompt, setPrompt] = useState('')
  const [count, setCount] = useState<number>(3)
  const [submitting, setSubmitting] = useState(false)
  const addGroup = useFanoutStore((s) => s.addGroup)
  const activeProjectId = useChatStore((s) => s.activeProjectId)

  const handleSubmit = async () => {
    const trimmed = prompt.trim()
    if (!trimmed) {
      toast.error(t('chat.fanout.emptyPrompt', { defaultValue: 'Enter a prompt first.' }))
      return
    }
    setSubmitting(true)
    try {
      const res = await api.post<FanoutResponse>('/api/worktree/fanout', {
        prompt: trimmed,
        count,
        project_path: projectPath ?? activeProjectId ?? '',
      })
      const groupId = res.group_id ?? `fanout-${Date.now()}`
      addGroup({
        groupId,
        prompt: trimmed,
        agents: (res.agents ?? []).map((a) => ({
          agentId: a.agent_id,
          name: a.name,
          worktreePath: a.worktree_path,
          status: 'working',
          detail: t('chat.fanout.starting', { defaultValue: 'Starting…' }),
          updatedAt: Date.now(),
        })),
      })
      toast.success(
        t('chat.fanout.launched', {
          defaultValue: 'Spawned {{count}} agents',
          count: res.agents?.length ?? count,
        }),
      )
      setOpen(false)
      setPrompt('')
      setCount(3)
    } catch (err) {
      toast.error(
        err instanceof Error
          ? err.message
          : t('chat.fanout.failed', { defaultValue: 'Fan-out failed' }),
      )
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <>
      <Button
        type="button"
        variant="outline"
        size="sm"
        onClick={() => setOpen(true)}
        disabled={submitting}
        className={className ?? 'h-8 gap-1 px-2 text-xs font-normal'}
        aria-label={t('chat.fanout.title', { defaultValue: 'Fan out to N agents' })}
        title={t('chat.fanout.title', { defaultValue: 'Fan out to N agents' })}
      >
        {submitting ? (
          <Loader2 className="size-3.5 animate-spin" />
        ) : (
          <GitFork className="size-3.5" />
        )}
        {showLabel && <span>{t('chat.fanout.label', { defaultValue: 'Fan Out' })}</span>}
      </Button>
      <Dialog open={open} onOpenChange={(o) => !submitting && setOpen(o)}>
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle>
              {t('chat.fanout.title', { defaultValue: 'Fan out to N agents' })}
            </DialogTitle>
            <DialogDescription>
              {t('chat.fanout.description', {
                defaultValue:
                  'Spawn multiple agents, each in its own worktree, to explore parallel approaches.',
              })}
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-3">
            <div className="space-y-1.5">
              <Label htmlFor="fanout-prompt">
                {t('chat.fanout.promptLabel', { defaultValue: 'Prompt' })}
              </Label>
              <Textarea
                id="fanout-prompt"
                rows={4}
                value={prompt}
                onChange={(e) => setPrompt(e.target.value)}
                placeholder={t('chat.fanout.promptPlaceholder', {
                  defaultValue: 'What should each agent try?',
                })}
                disabled={submitting}
                autoFocus
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="fanout-count">
                {t('chat.fanout.countLabel', { defaultValue: 'Number of agents' })}
              </Label>
              <SimpleSelect
                id="fanout-count"
                value={String(count)}
                onValueChange={(v: string) => setCount(Number(v))}
                options={COUNT_OPTIONS.map((n) => ({
                  value: String(n),
                  label: t('chat.fanout.countOption', {
                    defaultValue: '{{n}} agents',
                    n,
                  }),
                }))}
                disabled={submitting}
              />
            </div>
          </div>
          <DialogFooter>
            <Button
              type="button"
              variant="ghost"
              onClick={() => setOpen(false)}
              disabled={submitting}
            >
              {t('common.cancel', { defaultValue: 'Cancel' })}
            </Button>
            <Button
              type="button"
              onClick={handleSubmit}
              disabled={submitting || !prompt.trim()}
              className="gap-1.5"
            >
              {submitting && <Loader2 className="size-3.5 animate-spin" />}
              {submitting
                ? t('chat.fanout.launching', { defaultValue: 'Launching…' })
                : t('chat.fanout.launch', { defaultValue: 'Launch' })}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </>
  )
}
