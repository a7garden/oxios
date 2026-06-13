import { FileText } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { EmptyState } from '@/components/shared/empty-state'
import { Badge } from '@/components/ui/badge'
import type { AgentLogs as AgentLogsType } from '@/types/agent'

const levelColors: Record<string, string> = {
  info: 'bg-info-subtle text-info',
  warn: 'bg-warning-subtle text-warning',
  error: 'bg-error-subtle text-error',
  debug: 'bg-muted text-muted-foreground',
}

export function AgentLogs({ logs }: { logs: AgentLogsType | null | undefined }) {
  const { t } = useTranslation()

  if (!logs?.entries?.length) {
    return <EmptyState icon={<FileText className="h-10 w-10" />} title={t('agents.noLogs')} />
  }

  return (
    <div className="space-y-1 font-mono text-xs">
      {logs.entries.map((entry, i) => (
        <div key={i} className="flex items-start gap-2 py-1">
          <Badge
            variant="outline"
            className={`text-2xs shrink-0 ${levelColors[entry.level] || ''}`}
          >
            {entry.level}
          </Badge>
          <span className="text-muted-foreground shrink-0 tabular-nums">
            {entry.timestamp ? new Date(entry.timestamp).toLocaleTimeString() : ''}
          </span>
          <span className="break-all">{entry.message}</span>
        </div>
      ))}
    </div>
  )
}
