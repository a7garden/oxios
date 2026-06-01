import { useTranslation } from 'react-i18next'
import { useState } from 'react'
import { ChevronDown, ChevronRight, Wrench } from 'lucide-react'
import { useMcpTools } from '@/hooks/use-mcp'
import { ToolDetail } from '@/components/mcp/tool-detail'
import { EmptyState } from '@/components/shared/empty-state'
import { LoadingCards } from '@/components/shared/loading'
import { ErrorState } from '@/components/shared/error-state'
import { Input } from '@/components/ui/input'
import type { McpTool } from '@/types/mcp'

export function ToolList() {
  const { t } = useTranslation()
  const { data: tools, isLoading, isError, refetch } = useMcpTools()
  const [search, setSearch] = useState('')
  const [expanded, setExpanded] = useState<string | null>(null)

  if (isLoading) return <LoadingCards count={4} />
  if (isError) return <ErrorState onRetry={() => refetch()} />

  const filtered = (tools ?? []).filter(
    (tool) =>
      tool.name.toLowerCase().includes(search.toLowerCase()) ||
      tool.description.toLowerCase().includes(search.toLowerCase()) ||
      tool.server.toLowerCase().includes(search.toLowerCase()),
  )

  // Group by server
  const grouped = filtered.reduce<Record<string, McpTool[]>>((acc, tool) => {
    const key = tool.server || 'unknown'
    if (!acc[key]) acc[key] = []
    acc[key].push(tool)
    return acc
  }, {})

  if (!tools || tools.length === 0) {
    return (
      <EmptyState
        icon={<Wrench className="h-8 w-8" />}
        title={t('mcp.noTools', 'No MCP tools available')}
        description={t('mcp.noToolsDescription', 'Register and enable an MCP server to see its tools.')}
        className="py-6"
      />
    )
  }

  return (
    <div className="space-y-4">
      <Input
        value={search}
        onChange={(e) => setSearch(e.target.value)}
        placeholder={t('common.search', 'Search') + '...'}
        className="max-w-sm"
      />
      {Object.entries(grouped).length === 0 ? (
        <p className="text-sm text-muted-foreground py-4">{t('common.noData', 'No data')}</p>
      ) : (
        Object.entries(grouped).map(([server, serverTools]) => (
          <div key={server} className="space-y-1">
            <h3 className="text-sm font-medium text-muted-foreground uppercase tracking-wider px-2">
              {server} ({serverTools.length})
            </h3>
            <div className="space-y-1">
              {serverTools.map((tool) => {
                const key = `${server}::${tool.name}`
                const isExpanded = expanded === key
                return (
                  <div key={key} className="rounded-lg border">
                    <button
                      className="flex items-center gap-2 w-full p-3 text-left hover:bg-muted/50 transition-colors"
                      onClick={() => setExpanded(isExpanded ? null : key)}
                    >
                      {isExpanded ? (
                        <ChevronDown className="h-4 w-4 shrink-0" />
                      ) : (
                        <ChevronRight className="h-4 w-4 shrink-0" />
                      )}
                      <span className="font-mono text-sm font-medium">{tool.name}</span>
                      <span className="text-xs text-muted-foreground truncate flex-1">
                        {tool.description}
                      </span>
                    </button>
                    {isExpanded && (
                      <div className="border-t px-3 pt-3 pb-3">
                        <ToolDetail tool={tool} />
                      </div>
                    )}
                  </div>
                )
              })}
            </div>
          </div>
        ))
      )}
    </div>
  )
}
