/**
 * Memo settings card (oximemo first-party app module).
 *
 * Status + live Connect/Disconnect toggle — NOT a config-field section.
 * Mirrors the email setup flow: `GET /api/memo/status` (404 when the `memo`
 * cargo feature is absent → "not compiled" notice) and
 * `POST /api/memo/enable`|`disable` performing a live kernel swap. A full
 * memo-browsing panel is intentionally out of scope (oximemo ships its own UI).
 */
import { CheckCircle2, Loader2, Plug, PlugZap, XCircle } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { useMemoDisable, useMemoEnable, useMemoStatus } from '@/hooks/use-memo'
import { SectionCard } from './section-card'

export function MemoSectionCard() {
  const { t } = useTranslation()
  const { data: status, isLoading } = useMemoStatus()
  const enableMut = useMemoEnable()
  const disableMut = useMemoDisable()
  const [vaultPath, setVaultPath] = useState('')

  // Feature not compiled in (route 404s) → informational notice, no controls.
  if (!status) {
    return (
      <SectionCard
        title={t('settings.sectionMemo')}
        description={t('settings.memoDescription')}
        sectionId="memo"
      >
        <p className="text-muted-foreground px-4 py-3 text-sm">
          {isLoading ? '…' : t('settings.memoNotCompiled')}
        </p>
      </SectionCard>
    )
  }

  const connected = status.connected
  const busy = enableMut.isPending || disableMut.isPending

  const onEnable = () =>
    enableMut.mutate(vaultPath, {
      onSuccess: () => toast.success(t('settings.memoConnected')),
      onError: (e) => toast.error(e instanceof Error ? e.message : String(e)),
    })
  const onDisable = () =>
    disableMut.mutate(undefined, {
      onSuccess: () => toast.success(t('settings.memoDisconnected')),
      onError: (e) => toast.error(e instanceof Error ? e.message : String(e)),
    })

  return (
    <SectionCard
      title={t('settings.sectionMemo')}
      description={t('settings.memoDescription')}
      sectionId="memo"
    >
      <div className="flex items-center justify-between gap-4 px-4 py-3">
        <div className="min-w-0 space-y-1">
          <div className="flex items-center gap-2">
            <span
              className={`h-2 w-2 shrink-0 rounded-full ${
                connected ? 'bg-success' : 'bg-muted-foreground/40'
              }`}
            />
            {connected ? (
              <Badge className="border-success/40 bg-success/15 text-success">
                <CheckCircle2 className="mr-1 h-3 w-3" />
                {t('settings.memoConnectedBadge')}
              </Badge>
            ) : (
              <Badge variant="secondary">
                <XCircle className="mr-1 h-3 w-3" />
                {t('settings.memoDisconnectedBadge')}
              </Badge>
            )}
          </div>
          {status.vault_path && (
            <p className="truncate font-mono text-xs text-muted-foreground">{status.vault_path}</p>
          )}
        </div>
        {connected ? (
          <Button variant="outline" size="sm" onClick={onDisable} disabled={busy}>
            {disableMut.isPending ? (
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
            ) : (
              <PlugZap className="h-3.5 w-3.5" />
            )}
            <span className="ml-1.5">{t('settings.memoDisconnect')}</span>
          </Button>
        ) : (
          <Button size="sm" onClick={onEnable} disabled={busy}>
            {enableMut.isPending ? (
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
            ) : (
              <Plug className="h-3.5 w-3.5" />
            )}
            <span className="ml-1.5">{t('settings.memoConnect')}</span>
          </Button>
        )}
      </div>
      {!connected && (
        <div className="space-y-2 px-4 py-3">
          <label className="text-xs text-muted-foreground">{t('settings.memoVaultPath')}</label>
          <Input
            value={vaultPath}
            onChange={(e) => setVaultPath(e.target.value)}
            placeholder={t('settings.memoVaultPathPlaceholder')}
            className="font-mono text-xs"
          />
          <p className="text-xs text-muted-foreground">{t('settings.memoVaultPathHint')}</p>
        </div>
      )}
    </SectionCard>
  )
}
