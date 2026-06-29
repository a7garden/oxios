/**
 * Secrets management section (RFC-028 SP-2c).
 *
 * Lists all known secrets with masked status, allows setting and deleting
 * values via the `/api/secrets` backend. Secrets are stored in
 * `~/.oxi/auth.json`, never in `config.toml` plaintext.
 */
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { CheckCircle2, Eye, EyeOff, KeyRound, ShieldCheck, Trash2 } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { api } from '@/lib/api-client'
import { SectionCard } from './section-card'

interface SecretInfo {
  key: string
  has_value: boolean
  source: string
  preview: string
}

const SECRET_LABELS: Record<string, { labelKey: string; descKey: string }> = {
  telegram_bot_token: {
    labelKey: 'settings.secretTelegramToken',
    descKey: 'settings.secretTelegramTokenDesc',
  },
  email_smtp_password: {
    labelKey: 'settings.secretEmailPassword',
    descKey: 'settings.secretEmailPasswordDesc',
  },
  oxios_api_key: {
    labelKey: 'settings.secretOxiosApiKey',
    descKey: 'settings.secretOxiosApiKeyDesc',
  },
  clawhub_api_key: {
    labelKey: 'settings.secretClawhubApiKey',
    descKey: 'settings.secretClawhubApiKeyDesc',
  },
  anthropic: {
    labelKey: 'settings.secretAnthropicKey',
    descKey: 'settings.secretAnthropicKeyDesc',
  },
  openai: {
    labelKey: 'settings.secretOpenaiKey',
    descKey: 'settings.secretOpenaiKeyDesc',
  },
  google: {
    labelKey: 'settings.secretGoogleKey',
    descKey: 'settings.secretGoogleKeyDesc',
  },
}

/** Keys for which `/api/engine/validate-key` is meaningful. */
const PROVIDER_KEYS = new Set(['anthropic', 'openai', 'google', 'oxios_api_key', 'clawhub_api_key'])

function sourceBadgeClass(source: string): string {
  switch (source) {
    case 'env':
      return 'border-info/30 text-info'
    case 'auth_store':
      return 'border-success/30 text-success'
    case 'config':
      return 'border-warning/30 text-warning'
    default:
      return 'border-border text-muted-foreground'
  }
}

function sourceLabel(source: string, t: ReturnType<typeof useTranslation>['t']): string {
  switch (source) {
    case 'env':
      return t('settings.secretSourceEnv', 'Env Var')
    case 'auth_store':
      return t('settings.secretSourceStore', 'Auth Store')
    case 'config':
      return t('settings.secretSourceConfig', 'Config')
    default:
      return t('settings.secretSourceNone', 'Not Set')
  }
}
export function SecretsSectionCard() {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const [editValues, setEditValues] = useState<Record<string, string>>({})
  const [visibleKeys, setVisibleKeys] = useState<Set<string>>(new Set())

  const { data: secrets, isLoading } = useQuery<SecretInfo[]>({
    queryKey: ['secrets'],
    queryFn: () => api.get<SecretInfo[]>('/api/secrets'),
  })

  const validateMutation = useMutation({
    mutationFn: ({ key, value }: { key: string; value: string }) =>
      api.post<{ valid: boolean; message?: string }>('/api/engine/validate-key', {
        key,
        value,
      }),
    onSuccess: (res) => {
      if (res.valid) {
        toast.success(res.message ?? t('settings.secretValid', 'Secret is valid'))
      } else {
        toast.error(res.message ?? t('settings.secretInvalid', 'Secret is invalid'))
      }
    },
    onError: () =>
      toast.error(t('settings.secretValidateFailed', 'Failed to validate secret')),
  })

  const saveMutation = useMutation({
    mutationFn: ({ key, value }: { key: string; value: string }) =>
      api.put(`/api/secrets/${encodeURIComponent(key)}`, { value }),
    onSuccess: (_data, vars) => {
      queryClient.invalidateQueries({ queryKey: ['secrets'] })
      setEditValues((prev) => {
        const next = { ...prev }
        delete next[vars.key]
        return next
      })
      toast.success(t('settings.secretSaved', 'Secret saved'))
    },
    onError: () => toast.error(t('settings.secretSaveFailed', 'Failed to save secret')),
  })

  const deleteMutation = useMutation({
    mutationFn: (key: string) => api.delete(`/api/secrets/${encodeURIComponent(key)}`),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['secrets'] })
      toast.success(t('settings.secretDeleted', 'Secret deleted'))
    },
    onError: () => toast.error(t('settings.secretDeleteFailed', 'Failed to delete secret')),
  })

  const toggleVisible = (key: string) => {
    setVisibleKeys((prev) => {
      const next = new Set(prev)
      if (next.has(key)) next.delete(key)
      else next.add(key)
      return next
    })
  }

  return (
    <SectionCard
      title={t('settings.sectionSecrets', 'Secrets')}
      description={t('settings.secretsDescription', 'Manage API keys and credentials securely')}
      icon={<KeyRound className="h-3.5 w-3.5" />}
      sectionId="secrets"
      fieldCount={Object.keys(SECRET_LABELS).length}
      modified={false}
    >
      <div className="space-y-4">
        {isLoading && <p className="text-sm text-muted-foreground">Loading…</p>}
        {secrets?.map((secret) => {
          const labels = SECRET_LABELS[secret.key]
          if (!labels) return null
          const isEditing = editValues[secret.key] !== undefined
          const isVisible = visibleKeys.has(secret.key)

          return (
            <div key={secret.key} className="space-y-1.5">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <label className="text-sm font-medium">{t(labels.labelKey, secret.key)}</label>
                  {secret.has_value && (
                    <Badge
                      variant="outline"
                      className={`text-2xs ${sourceBadgeClass(secret.source)}`}
                    >
                      {sourceLabel(secret.source, t)}
                    </Badge>
                  )}
                  {secret.has_value && secret.preview && (
                    <span className="text-xs font-mono text-muted-foreground">
                      {secret.preview}
                    </span>
                  )}
                </div>
                {secret.has_value && !isEditing && (
                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-7 text-xs"
                    onClick={() => {
                      if (window.confirm(t('settings.secretDeleteConfirm', 'Delete this secret?')))
                        deleteMutation.mutate(secret.key)
                    }}
                  >
                    <Trash2 className="h-3 w-3 mr-1" />
                    {t('common.delete', 'Delete')}
                  </Button>
                )}
              </div>

              {isEditing ? (
                <div className="flex gap-2">
                  <Input
                    type={isVisible ? 'text' : 'password'}
                    value={editValues[secret.key] ?? ''}
                    onChange={(e) =>
                      setEditValues((prev) => ({ ...prev, [secret.key]: e.target.value }))
                    }
                    placeholder={t('settings.secretEnterValue', 'Enter value…')}
                    className="flex-1"
                  />
                  <Button
                    variant="ghost"
                    size="icon"
                    className="h-9 w-9"
                    onClick={() => toggleVisible(secret.key)}
                  >
                    {isVisible ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
                  </Button>
                  <Button
                    size="sm"
                    onClick={() =>
                      saveMutation.mutate({ key: secret.key, value: editValues[secret.key] ?? '' })
                    }
                    disabled={!editValues[secret.key] || saveMutation.isPending}
                  >
                    <CheckCircle2 className="h-3.5 w-3.5 mr-1" />
                    {t('common.save', 'Save')}
                  </Button>
                  {PROVIDER_KEYS.has(secret.key) && (
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() =>
                        validateMutation.mutate({
                          key: secret.key,
                          value: editValues[secret.key] ?? '',
                        })
                      }
                      disabled={
                        !editValues[secret.key] || validateMutation.isPending
                      }
                    >
                      <ShieldCheck className="h-3.5 w-3.5 mr-1" />
                      {t('settings.verify', 'Verify')}
                    </Button>
                  )}
                </div>
              ) : (
                <div className="flex gap-2">
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => setEditValues((prev) => ({ ...prev, [secret.key]: '' }))}
                  >
                    {secret.has_value
                      ? t('settings.secretUpdate', 'Update')
                      : t('settings.secretSet', 'Set Value')}
                  </Button>
                </div>
              )}
              <p className="text-xs text-muted-foreground">{t(labels.descKey, '')}</p>
            </div>
          )
        })}
      </div>
    </SectionCard>
  )
}
