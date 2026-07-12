import { Pencil, Power, RefreshCw, Trash2 } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import type { McpServer } from '@/types/mcp'

interface ServerCardProps {
  server: McpServer
  onToggle: () => void
  onRefresh: () => void
  onDelete: () => void
  onEdit: () => void
  isToggling: boolean
  isRefreshing: boolean
  isDeleting: boolean
}

export function ServerCard({
  server,
  onToggle,
  onRefresh,
  onDelete,
  onEdit,
  isToggling,
  isRefreshing,
  isDeleting,
}: ServerCardProps) {
  const { t } = useTranslation()

  const statusColor = !server.enabled
    ? 'bg-muted-foreground'
    : server.initialized
      ? 'bg-success'
      : 'bg-error'

  const statusText = !server.enabled
    ? t('common.disabled')
    : server.initialized
      ? t('mcp.connected')
      : t('mcp.disconnected')

  return (
    <div className="flex items-center gap-4 rounded-lg border p-4">
      <div className="flex items-center gap-3 flex-1 min-w-0">
        <div className={`h-2.5 w-2.5 rounded-full shrink-0 ${statusColor}`} title={statusText} />
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <span className="font-medium text-sm truncate">{server.name}</span>
            <Badge variant={server.enabled ? 'default' : 'secondary'} className="text-xs">
              {statusText}
            </Badge>
          </div>
          <p className="text-xs text-muted-foreground font-mono truncate mt-0.5">
            {server.command}
            {server.args.length > 0 ? ` ${server.args.join(' ')}` : ''}
          </p>
        </div>
      </div>

      <div className="flex items-center gap-1.5 shrink-0">
        <Button
          variant="ghost"
          size="icon"
          className="h-10 w-10"
          onClick={onRefresh}
          disabled={isRefreshing || !server.enabled}
          title={t('mcp.refresh')}
        >
          <RefreshCw className={`h-4 w-4 ${isRefreshing ? 'animate-spin' : ''}`} />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className="h-10 w-10"
          onClick={onEdit}
          title={t('mcp.edit')}
        >
          <Pencil className="h-4 w-4" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className="h-10 w-10"
          onClick={onToggle}
          disabled={isToggling}
          title={server.enabled ? t('mcp.disable') : t('mcp.enable')}
        >
          <Power className="h-4 w-4" />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className="h-10 w-10 text-destructive hover:text-destructive"
          onClick={onDelete}
          disabled={isDeleting}
          title={t('mcp.remove')}
        >
          <Trash2 className="h-4 w-4" />
        </Button>
      </div>
    </div>
  )
}
