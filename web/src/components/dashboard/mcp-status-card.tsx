import { Link } from '@tanstack/react-router'
import { Plug, PlugZap } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { useMcpServers, useMcpTools } from '@/hooks/use-mcp'

/**
 * MCP server status card for the dashboard.
 *
 * Shows connected/total MCP servers, active tool count,
 * and per-server status indicators.
 */
export function McpStatusCard({ className }: { className?: string }) {
  const { t } = useTranslation()
  const { data: servers } = useMcpServers()
  const { data: tools } = useMcpTools()

  const allServers = Array.isArray(servers) ? servers : []
  const activeServers = allServers.filter((s) => s.enabled && s.initialized)
  const inactiveServers = allServers.filter((s) => !s.enabled || !s.initialized)
  const totalTools = tools?.length ?? 0

  return (
    <Card className={className}>
      <CardHeader className="flex flex-row items-center justify-between pb-2">
        <CardTitle className="flex items-center gap-2 text-base">
          <Plug className="h-4 w-4" />
          {t('dashboard.mcpServers')}
        </CardTitle>
        <Link
          to="/mcp"
          search={{ tab: undefined }}
          className="text-xs text-muted-foreground hover:text-foreground underline-offset-4 hover:underline"
        >
          {t('dashboard.viewAll')}
        </Link>
      </CardHeader>
      <CardContent className="pt-0">
        {allServers.length === 0 ? (
          <p className="text-xs text-muted-foreground py-1">{t('dashboard.noMcpServers')}</p>
        ) : (
          <div className="space-y-1.5">
            <div className="flex items-center justify-between text-xs">
              <span className="text-muted-foreground">{t('dashboard.activeConnections')}</span>
              <span className="font-semibold">
                {activeServers.length}/{allServers.length}
              </span>
            </div>
            <div className="flex items-center justify-between text-xs">
              <span className="text-muted-foreground">{t('dashboard.availableTools')}</span>
              <span className="font-semibold">{totalTools}</span>
            </div>
            <div className="pt-1 space-y-1">
              {activeServers.map((s) => (
                <div key={s.name} className="flex items-center gap-2 text-xs">
                  <div className="h-1.5 w-1.5 rounded-full bg-success" />
                  <span className="truncate font-medium">{s.name}</span>
                </div>
              ))}
              {inactiveServers.map((s) => (
                <div key={s.name} className="flex items-center gap-2 text-xs text-muted-foreground">
                  <PlugZap className="h-3 w-3" />
                  <span className="truncate">{s.name}</span>
                </div>
              ))}
            </div>
          </div>
        )}
      </CardContent>
    </Card>
  )
}
