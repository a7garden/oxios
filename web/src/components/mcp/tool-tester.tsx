import { Play } from 'lucide-react'
import { useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { Label } from '@/components/ui/label'
import { Select } from '@/components/ui/select'
import { Textarea } from '@/components/ui/textarea'
import { useMcpCallTool, useMcpServers, useMcpTools } from '@/hooks/use-mcp'
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

  const enabledServers = useMemo(() => (servers ?? []).filter((s) => s.enabled), [servers])

  const serverTools = useMemo<McpTool[]>(
    () => (selectedServer ? (tools ?? []).filter((tool) => tool.server === selectedServer) : []),
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
    <div className="space-y-4 max-w-3xl">
      {/* Server selector */}
      <div className="space-y-2">
        <Label>{t('mcp.servers')}</Label>
        <Select
          value={selectedServer}
          onValueChange={(v) => {
            setSelectedServer(v)
            setSelectedTool('')
          }}
          placeholder={t('common.selectPlaceholder')}
          options={enabledServers.map((s) => ({ label: s.name, value: s.name }))}
        />
      </div>

      {/* Tool selector */}
      <div className="space-y-2">
        <Label>{t('mcp.tools')}</Label>
        <Select
          value={selectedTool}
          onValueChange={setSelectedTool}
          placeholder={t('common.selectPlaceholder')}
          options={serverTools.map((tool) => ({ label: tool.name, value: tool.name }))}
          className={!selectedServer ? 'pointer-events-none opacity-50' : ''}
        />
      </div>

      {/* Tool description */}
      {selectedTool &&
        serverTools.length > 0 &&
        (() => {
          const tool = serverTools.find((t) => t.name === selectedTool)
          if (!tool) return null
          return <p className="text-xs text-muted-foreground">{tool.description}</p>
        })()}

      {/* Arguments */}
      <div className="space-y-2">
        <Label>{t('mcp.args')} (JSON)</Label>
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
        {callTool.isPending ? t('common.loading') : t('mcp.execute')}
      </Button>

      {/* Duration */}
      {duration !== null && (
        <p className="text-xs text-muted-foreground">
          {t('mcp.duration')}: {duration}ms
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
          <Label>{t('mcp.result')}</Label>
          <pre className="rounded-lg border bg-muted p-3 text-xs font-mono overflow-x-auto max-h-80 overflow-y-auto whitespace-pre-wrap">
            {result}
          </pre>
        </div>
      )}
    </div>
  )
}
