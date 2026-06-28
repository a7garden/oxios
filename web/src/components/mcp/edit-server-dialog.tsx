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
        toast.error(
          t('mcp.envParseError', '환경변수 형식이 잘못되었습니다. KEY=VALUE 형식이어야 합니다.'),
        )
        return
      }
      const key = trimmed.slice(0, eq).trim()
      const value = trimmed.slice(eq + 1).trim()
      if (!key) {
        toast.error(
          t('mcp.envParseError', '환경변수 형식이 잘못되었습니다. KEY=VALUE 형식이어야 합니다.'),
        )
        return
      }
      if (value === '') {
        toast.error(
          t(
            'mcp.envEmptyValue',
            '환경변수 값이 비어있습니다. 값을 지정하거나 해당 줄을 삭제하세요.',
          ),
        )
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
          toast.success(t('mcp.updated', 'MCP 서버가 업데이트되었습니다'))
          close()
        },
        onError: (err) => {
          toast.error(err instanceof Error ? err.message : t('mcp.updateFailed', '업데이트 실패'))
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
            {t('mcp.edit', 'MCP 서버 편집')}
          </DialogTitle>
          <DialogDescription>
            {t('mcp.editDescription', '서버의 명령, 인자, 환경변수를 변경합니다. 저장 시 재시작됩니다.')}
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="space-y-4">
          <div className="space-y-2">
            <Label>{t('mcp.name', '이름')}</Label>
            <Input value={server?.name ?? ''} disabled className="font-mono" />
            <p className="text-xs text-muted-foreground">
              {t('mcp.nameImmutable', '이름은 변경할 수 없습니다. 변경하려면 삭제 후 새로 등록하세요.')}
            </p>
          </div>
          <div className="space-y-2">
            <Label htmlFor="mcp-edit-command">{t('mcp.command', '명령')}</Label>
            <Input
              id="mcp-edit-command"
              value={command}
              onChange={(e) => setCommand(e.target.value)}
              className="font-mono"
              autoFocus
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="mcp-edit-args">{t('mcp.args', '인자 (콤마 구분)')}</Label>
            <Input
              id="mcp-edit-args"
              value={args}
              onChange={(e) => setArgs(e.target.value)}
              placeholder="-y, @anthropic/mcp-server-filesystem"
              className="font-mono"
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="mcp-edit-env">
              {t('mcp.env', '환경변수 (KEY=VALUE, 줄바꿈 구분)')}
            </Label>
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
            {t('common.enabled', '활성화')}
          </label>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={close}>
              {t('common.cancel', '취소')}
            </Button>
            <Button type="submit" disabled={!command.trim() || updateServer.isPending}>
              {updateServer.isPending
                ? t('common.saving', '저장 중...')
                : t('common.save', '저장')}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
