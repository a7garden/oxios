import { useTranslation } from 'react-i18next'
import type { McpTool } from '@/types/mcp'

interface ToolDetailProps {
  tool: McpTool
}

export function ToolDetail({ tool }: ToolDetailProps) {
  const { t } = useTranslation()

  return (
    <div className="space-y-3 pt-2">
      <div>
        <p className="text-sm text-muted-foreground">{tool.description}</p>
      </div>
      {tool.arguments && tool.arguments.length > 0 && (
        <div>
          <h4 className="text-xs font-medium text-muted-foreground uppercase tracking-wider mb-2">
            {t('mcp.args')}
          </h4>
          <div className="space-y-1.5">
            {tool.arguments.map((arg) => (
              <div key={arg.name} className="flex items-start gap-2 text-sm">
                <code className="font-mono text-xs bg-muted px-1.5 py-0.5 rounded">{arg.name}</code>
                {arg.required && (
                  <span className="text-xs text-destructive font-medium">required</span>
                )}
                {arg.type && <span className="text-xs text-muted-foreground">{arg.type}</span>}
                {arg.description && (
                  <span className="text-xs text-muted-foreground flex-1">— {arg.description}</span>
                )}
              </div>
            ))}
          </div>
        </div>
      )}
      {(!tool.arguments || tool.arguments.length === 0) && (
        <p className="text-xs text-muted-foreground">{t('mcp.noArguments')}</p>
      )}
    </div>
  )
}
