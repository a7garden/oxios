import { Pencil } from 'lucide-react'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { useMcpUpdateServer } from '@/hooks/use-mcp'
import type { McpServer, McpServerUpdateRequest } from '@/types/mcp'

interface EditMcpServerDialogProps {
  server: McpServer | null
  onOpenChange: (open: boolean) => void
}

/**
 * MCP server 편집 다이얼로그. 백엔드 PUT /api/mcp/servers/:name 으로
 * command/args/env/enabled 를 갱신하면 서버가 재시작됩니다.
 *
 * args 는 콤마 구분 문자열, env 는 줄바꿈 구분 `KEY=VALUE` 형식입니다.
 */
export function EditMcpServerDialog({ server, onOpenChange }: EditMcpServerDialogProps) {
  const { t } = useTranslation()
  const [command, setCommand] = useState('')
  const [args, setArgs] = useState('')
  const [envText, setEnvText] = useState('')
  const [enabled, setEnabled] = useState(true)
  const updateServer = useMcpUpdateServer()

  useEffect(() => {
    if (server) {
      setCommand(server.command)
      setArgs(server.args.join(', '))
      setEnvText(
        Object.entries(server.env ?? {})
          .map(([k, v]) => `${k}=${v}`)
          .join('\n'),
      )
      setEnabled(server.enabled)
    }
  }, [server])

  const close = () => onOpenChange(false)

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (!server) return
    const parsedArgs = args
      .split(',')
      .map((a) => a.trim())
      .filter(Boolean)
    const parsedEnv: Record<string, string> = {}
    for (const line of envText.split('\n')) {
      const trimmed = line.trim()
      if (!trimmed) continue
      const eq = trimmed.indexOf('=')
      if (eq <= 0) {
        toast.error(t('mcp.envParseError'))
        return
      }
      const key = trimmed.slice(0, eq).trim()
      const value = trimmed.slice(eq + 1).trim()
      if (!key) {
        toast.error(t('mcp.envParseError'))
        return
      }
      if (value === '') {
        toast.error(t('mcp.envEmptyValue'))
        return
      }
      parsedEnv[key] = value
    }
    const body: McpServerUpdateRequest = {
      command: command.trim(),
      args: parsedArgs,
      env: parsedEnv,
      enabled,
    }
    updateServer.mutate(
      { name: server.name, body },
      {
        onSuccess: () => {
          toast.success(t('mcp.updated'))
          close()
        },
        onError: (err) => {
          toast.error(err instanceof Error ? err.message : t('mcp.updateFailed'))
        },
      },
    )
  }

  return (
    <Dialog open={server !== null} onOpenChange={(o) => !o && close()}>
      <DialogContent className="max-w-lg">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Pencil className="h-5 w-5" />
            {t('mcp.edit')}
          </DialogTitle>
          <DialogDescription>{t('mcp.editDescription')}</DialogDescription>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="space-y-4">
          <div className="space-y-2">
            <Label>{t('mcp.name')}</Label>
            <Input value={server?.name ?? ''} disabled className="font-mono" />
            <p className="text-xs text-muted-foreground">{t('mcp.nameImmutable')}</p>
          </div>
          <div className="space-y-2">
            <Label htmlFor="mcp-edit-command">{t('mcp.command')}</Label>
            <Input
              id="mcp-edit-command"
              value={command}
              onChange={(e) => setCommand(e.target.value)}
              className="font-mono"
              autoFocus
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="mcp-edit-args">{t('mcp.args')}</Label>
            <Input
              id="mcp-edit-args"
              value={args}
              onChange={(e) => setArgs(e.target.value)}
              placeholder="-y, @anthropic/mcp-server-filesystem"
              className="font-mono"
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="mcp-edit-env">{t('mcp.env')}</Label>
            <textarea
              id="mcp-edit-env"
              value={envText}
              onChange={(e) => setEnvText(e.target.value)}
              rows={3}
              className="flex w-full rounded-md border border-input bg-background px-3 py-2 text-sm font-mono ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              placeholder="API_KEY=xxx&#10;LOG_LEVEL=info"
            />
          </div>
          <label className="flex items-center gap-2 text-sm">
            <input
              type="checkbox"
              checked={enabled}
              onChange={(e) => setEnabled(e.target.checked)}
              className="h-4 w-4 rounded border-input"
            />
            {t('common.enabled')}
          </label>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={close}>
              {t('common.cancel')}
            </Button>
            <Button type="submit" disabled={!command.trim() || updateServer.isPending}>
              {updateServer.isPending ? t('common.saving') : t('common.save')}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
