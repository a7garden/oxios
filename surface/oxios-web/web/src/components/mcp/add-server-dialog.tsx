import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { useMcpRegisterServer } from '@/hooks/use-mcp'

interface AddServerDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

export function AddServerDialog({ open, onOpenChange }: AddServerDialogProps) {
  const { t } = useTranslation()
  const [name, setName] = useState('')
  const [command, setCommand] = useState('')
  const [args, setArgs] = useState('')

  const registerServer = useMcpRegisterServer()

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (!name.trim() || !command.trim()) return

    const parsedArgs = args.trim()
      ? args
          .split(',')
          .map((a) => a.trim())
          .filter(Boolean)
      : []

    registerServer.mutate(
      { name: name.trim(), command: command.trim(), args: parsedArgs },
      {
        onSuccess: () => {
          setName('')
          setCommand('')
          setArgs('')
          onOpenChange(false)
        },
      },
    )
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{t('mcp.addServer', 'Add MCP Server')}</DialogTitle>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="mcp-name">{t('mcp.serverName', 'Server Name')}</Label>
            <Input
              id="mcp-name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder={t('mcp.serverNamePlaceholder', 'e.g. filesystem')}
              required
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="mcp-command">{t('mcp.command', 'Command')}</Label>
            <Input
              id="mcp-command"
              value={command}
              onChange={(e) => setCommand(e.target.value)}
              placeholder={t(
                'mcp.commandPlaceholder',
                'e.g. npx -y @modelcontextprotocol/server-filesystem',
              )}
              required
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="mcp-args">{t('mcp.args', 'Arguments')}</Label>
            <Input
              id="mcp-args"
              value={args}
              onChange={(e) => setArgs(e.target.value)}
              placeholder={t(
                'mcp.argsPlaceholder',
                'Comma-separated, e.g. /path/to/dir, --verbose',
              )}
            />
          </div>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => onOpenChange(false)}>
              {t('common.cancel', 'Cancel')}
            </Button>
            <Button
              type="submit"
              disabled={!name.trim() || !command.trim() || registerServer.isPending}
            >
              {registerServer.isPending
                ? t('common.loading', 'Loading...')
                : t('mcp.register', 'Register')}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
