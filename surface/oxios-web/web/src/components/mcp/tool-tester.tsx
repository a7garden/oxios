import { useTranslation } from 'react-i18next'
import { useState, useMemo } from 'react'
import { Play } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import { Textarea } from '@/components/ui/textarea'
import { useMcpServers, useMcpTools, useMcpCallTool } from '@/hooks/use-mcp'
import type { McpTool } from '@/types/mcp'

export function ToolTester() {
  const { t } = useTranslation()
  const { data: servers } = useMcpServers()
  const { data: tools } = useMcpTools()
  const callTool = useMcpCallTool()

  const [selectedServer, setSelectedServer] = useState('')
  const [selectedTool, setSelectedTool] = useState('')
  const [argsJson, setArgsJson] = useState('{}')
  const [result, setResult] = useState<string | null>(null)
  const [duration, setDuration] = useState<number | null>(null)
  const [error, setError] = useState<string | null>(null)

  const enabledServers = useMemo(
    () => (servers ?? []).filter((s) => s.enabled),
    [servers],
  )

  const serverTools = useMemo<McpTool[]>(
    () =>
      selectedServer
        ? (tools ?? []).filter((tool) => tool.server === selectedServer)
        : [],
    [tools, selectedServer],
  )

  const handleExecute = async () => {
    if (!selectedServer || !selectedTool) return

    let parsedArgs: Record<string, unknown>
    try {
      parsedArgs = JSON.parse(argsJson)
    } catch {
      setError('Invalid JSON')
      setResult(null)
      return
    }

    setError(null)
    setResult(null)
    setDuration(null)

    const start = performance.now()
    try {
      const res = await callTool.mutateAsync({
        server: selectedServer,
        tool: selectedTool,
        arguments: parsedArgs,
      })
      const elapsed = Math.round(performance.now() - start)
      setDuration(elapsed)
      setResult(JSON.stringify(res, null, 2))
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
    }
  }

  return (
    <div className="space-y-4">
      {/* Server selector */}
      <div className="space-y-2">
        <Label>{t('mcp.servers', 'Server')}</Label>
        <select
          value={selectedServer}
          onChange={(e) => {
            setSelectedServer(e.target.value)
            setSelectedTool('')
          }}
          className="flex h-9 w-full rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
        >
          <option value="">{t('common.selectPlaceholder', 'Select...')}</option>
          {enabledServers.map((s) => (
            <option key={s.name} value={s.name}>{s.name}</option>
          ))}
        </select>
      </div>

      {/* Tool selector */}
      <div className="space-y-2">
        <Label>{t('mcp.tools', 'Tool')}</Label>
        <select
          value={selectedTool}
          onChange={(e) => setSelectedTool(e.target.value)}
          disabled={!selectedServer}
          className="flex h-9 w-full rounded-md border border-input bg-transparent px-3 py-1 text-sm shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:opacity-50"
        >
          <option value="">{t('common.selectPlaceholder', 'Select...')}</option>
          {serverTools.map((tool) => (
            <option key={tool.name} value={tool.name}>{tool.name}</option>
          ))}
        </select>
      </div>

      {/* Tool description */}
      {selectedTool && serverTools.length > 0 && (
        (() => {
          const tool = serverTools.find((t) => t.name === selectedTool)
          if (!tool) return null
          return (
            <p className="text-xs text-muted-foreground">{tool.description}</p>
          )
        })()
      )}

      {/* Arguments */}
      <div className="space-y-2">
        <Label>{t('mcp.args', 'Arguments')} (JSON)</Label>
        <Textarea
          value={argsJson}
          onChange={(e) => setArgsJson(e.target.value)}
          rows={5}
          className="font-mono text-sm"
          placeholder='{"key": "value"}'
        />
      </div>

      {/* Execute */}
      <Button
        onClick={handleExecute}
        disabled={!selectedServer || !selectedTool || callTool.isPending}
      >
        <Play className="h-4 w-4 mr-1" />
        {callTool.isPending ? t('common.loading', 'Loading...') : t('mcp.execute', 'Execute')}
      </Button>

      {/* Duration */}
      {duration !== null && (
        <p className="text-xs text-muted-foreground">
          {t('mcp.duration', 'Duration')}: {duration}ms
        </p>
      )}

      {/* Error */}
      {error && (
        <div className="rounded-lg border border-destructive/50 bg-destructive/10 p-3">
          <p className="text-sm text-destructive font-medium">Error</p>
          <p className="text-sm text-destructive/80 mt-1">{error}</p>
        </div>
      )}

      {/* Result */}
      {result && (
        <div className="space-y-2">
          <Label>{t('mcp.result', 'Result')}</Label>
          <pre className="rounded-lg border bg-muted p-3 text-xs font-mono overflow-x-auto max-h-80 overflow-y-auto whitespace-pre-wrap">
            {result}
          </pre>
        </div>
      )}
    </div>
  )
}
