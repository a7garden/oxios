import { useTranslation } from 'react-i18next'
import { Power, RefreshCw, Trash2 } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { ServerCard } from '@/components/mcp/server-card'
import { useMcpServers, useMcpDeleteServer, useMcpToggleServer, useMcpRefreshServer } from '@/hooks/use-mcp'
import { EmptyState } from '@/components/shared/empty-state'
import { LoadingCards } from '@/components/shared/loading'
import { ErrorState } from '@/components/shared/error-state'
import { Server } from 'lucide-react'

export function ServerList() {
  const { t } = useTranslation()
  const { data: servers, isLoading, isError, refetch } = useMcpServers()
  const deleteServer = useMcpDeleteServer()
  const toggleServer = useMcpToggleServer()
  const refreshServer = useMcpRefreshServer()

  if (isLoading) return <LoadingCards count={3} />
  if (isError) return <ErrorState onRetry={() => refetch()} />

  if (!servers || servers.length === 0) {
    return (
      <EmptyState
        icon={<Server className="h-8 w-8" />}
        title={t('mcp.noServers', 'No MCP servers configured')}
        description={t('mcp.noServersDescription', 'Add an MCP server to get started.')}
        className="py-6"
      />
    )
  }

  return (
    <div className="space-y-3">
      {servers.map((server) => (
        <ServerCard
          key={server.name}
          server={server}
          onToggle={() => toggleServer.mutate(server.name)}
          onRefresh={() => refreshServer.mutate(server.name)}
          onDelete={() => deleteServer.mutate(server.name)}
          isToggling={toggleServer.isPending}
          isRefreshing={refreshServer.isPending}
          isDeleting={deleteServer.isPending}
        />
      ))}
    </div>
  )
}
