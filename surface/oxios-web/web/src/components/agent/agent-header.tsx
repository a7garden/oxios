import { Bot, ArrowLeft } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { StatusIndicator } from '@/components/shared/status-indicator'
import type { AgentDetail } from '@/types/agent'

interface AgentHeaderProps {
  agent: AgentDetail
  onBack: () => void
  children?: React.ReactNode
}

export function AgentHeader({ agent, onBack, children }: AgentHeaderProps) {
  const { t } = useTranslation()
  return (
    <div className="space-y-3">
      <div className="flex items-center gap-4">
        <Button variant="ghost" size="icon" onClick={onBack} aria-label={t('common.back')}>
          <ArrowLeft className="h-4 w-4" />
        </Button>
        <div className="flex-1">
          <h1 className="text-2xl font-bold flex items-center gap-2">
            <Bot className="h-6 w-6" /> {agent.name}
          </h1>
          <p className="text-muted-foreground font-mono text-xs">{agent.id}</p>
        </div>
        <StatusIndicator status={agent.status?.toLowerCase() ?? 'unknown'} />
        {children}
      </div>
    </div>
  )
}
