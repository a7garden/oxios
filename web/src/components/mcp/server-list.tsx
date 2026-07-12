import { Server } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { EditMcpServerDialog } from '@/components/mcp/edit-server-dialog'
import { ServerCard } from '@/components/mcp/server-card'
import { EmptyState } from '@/components/shared/empty-state'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import {
  useMcpDeleteServer,
  useMcpRefreshServer,
  useMcpServers,
  useMcpToggleServer,
} from '@/hooks/use-mcp'
import type { McpServer } from '@/types/mcp'

export function ServerList() {
  const { t } = useTranslation()
  const { data: servers, isLoading, isError, refetch } = useMcpServers()
  const deleteServer = useMcpDeleteServer()
  const toggleServer = useMcpToggleServer()
  const refreshServer = useMcpRefreshServer()
  const [editing, setEditing] = useState<McpServer | null>(null)

  if (isLoading) return <LoadingCards count={3} />
  if (isError) return <ErrorState onRetry={() => refetch()} />

  if (!servers || servers.length === 0) {
    return (
      <>
        <EmptyState
          icon={<Server className="h-8 w-8" />}
          title={t('mcp.noServers')}
          description={t('mcp.noServersDescription')}
          className="py-6"
        />
        <EditMcpServerDialog server={editing} onOpenChange={(o) => !o && setEditing(null)} />
      </>
    )
  }

  return (
    <>
      <div className="space-y-3">
        {servers.map((server) => (
          <ServerCard
            key={server.name}
            server={server}
            onToggle={() => toggleServer.mutate(server.name)}
            onRefresh={() => refreshServer.mutate(server.name)}
            onDelete={() => deleteServer.mutate(server.name)}
            onEdit={() => setEditing(server)}
            isToggling={toggleServer.isPending}
            isRefreshing={refreshServer.isPending}
            isDeleting={deleteServer.isPending}
          />
        ))}
      </div>
      <EditMcpServerDialog server={editing} onOpenChange={(o) => !o && setEditing(null)} />
    </>
  )
}
