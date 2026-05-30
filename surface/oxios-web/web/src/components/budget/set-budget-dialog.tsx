import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import type { AgentBudget, SetBudgetRequest } from '@/types/budget'

interface Props {
  open: boolean
  onOpenChange: (open: boolean) => void
  agent?: AgentBudget | null
  agentId?: string
  onSubmit: (data: { agentId: string } & SetBudgetRequest) => void
  isPending: boolean
}

export function SetBudgetDialog({ open, onOpenChange, agent, agentId, onSubmit, isPending }: Props) {
  const { t } = useTranslation()
  const targetId = agent?.agent_id ?? agentId ?? ''
  const displayName = agent?.name || targetId.slice(0, 12) + '...'

  const [tokenBudget, setTokenBudget] = useState(agent?.budget.token_limit?.toString() ?? '50000')
  const [callsBudget, setCallsBudget] = useState(agent?.budget.calls_limit?.toString() ?? '100')
  const [windowSecs, setWindowSecs] = useState(agent?.budget.window_secs?.toString() ?? '3600')

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    onSubmit({
      agentId: targetId,
      token_budget: parseInt(tokenBudget, 10) || 0,
      calls_budget: parseInt(callsBudget, 10) || 0,
      window_secs: parseInt(windowSecs, 10) || 3600,
    })
    onOpenChange(false)
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{t('budget.setBudgetFor', { agent: displayName })}</DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="space-y-4">
          <div className="space-y-2">
            <label className="text-sm font-medium">{t('budget.tokenLimit')}</label>
            <Input
              type="number"
              value={tokenBudget}
              onChange={(e) => setTokenBudget(e.target.value)}
              min={0}
            />
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium">{t('budget.callLimit')}</label>
            <Input
              type="number"
              value={callsBudget}
              onChange={(e) => setCallsBudget(e.target.value)}
              min={0}
            />
          </div>
          <div className="space-y-2">
            <label className="text-sm font-medium">{t('budget.windowSec')}</label>
            <Input
              type="number"
              value={windowSecs}
              onChange={(e) => setWindowSecs(e.target.value)}
              min={1}
            />
          </div>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              {t('common.cancel')}
            </Button>
            <Button type="submit" disabled={isPending}>
              {t('common.save')}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
