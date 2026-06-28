import { Check, KeyRound } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover'
import { useProviderQuotas } from '@/hooks/use-costs'
import { useProviders } from '@/hooks/use-engine'
import { cn } from '@/lib/utils'

export interface RolePillProps {
  /** All roles from useRoles() (name → model id). */
  roles: { name: string; model: string }[]
  /** Currently active role name, or null for default. */
  activeRole: string | null
  /** Setter. */
  onChange: (role: string | null) => void
  /** Show only the default-model row? (false once a real role is picked) */
  hasRoles: boolean
}

/**
 * RolePill — replaces the bare <select> in ChatInput. Built on shadcn
 * Popover (the legacy Select in this project doesn't allow rich trigger
 * content or row icons, so we use Popover + a custom button list).
 *
 * The pill shows the current routing target as a compact chip and on
 * click opens a popover with the full role list. Each row carries:
 *   - a status dot (green = provider has API key, amber = not configured)
 *   - the role name
 *   - the model short id (last segment of `provider/model`)
 *   - a key icon when the provider is missing its API key
 *   - a check icon on the active row
 *
 * Falls back to a static "default model" pill when no roles are
 * configured — identical to the prior <select>'s empty case so the
 * layout never collapses.
 */
export function RolePill({ roles, activeRole, onChange, hasRoles }: RolePillProps) {
  const { t } = useTranslation()
  const { data: providers } = useProviders()
  const { data: quotaData } = useProviderQuotas()

  // Build a lookup: provider id → { configured, quota }
  const providerMap = new Map<string, { configured: boolean; quota: number | null }>()
  for (const p of providers ?? []) {
    providerMap.set(p.id, { configured: p.hasKey, quota: null })
  }
  for (const q of quotaData?.providers ?? []) {
    const entry = providerMap.get(q.provider)
    if (entry) entry.quota = q.credit_balance_usd ?? null
  }

  // Resolve the currently active role → provider id (first path segment of `provider/model`)
  const currentEntry = activeRole ? (roles.find((r) => r.name === activeRole) ?? null) : null
  const currentProviderId = currentEntry?.model.includes('/')
    ? currentEntry.model.split('/')[0]
    : null
  const currentShortModel = currentEntry?.model
    ? currentEntry.model.includes('/')
      ? (currentEntry.model.split('/').pop() ?? currentEntry.model)
      : currentEntry.model
    : null
  const currentProvider = currentProviderId ? providerMap.get(currentProviderId) : undefined

  // "Default model" — no roles configured OR user explicitly picked default.
  if (!hasRoles || roles.length === 0) {
    return (
      <div
        className="inline-flex items-center gap-1.5 h-7 max-w-[200px] truncate rounded-md border border-dashed border-input bg-muted/30 px-2 text-2xs text-muted-foreground"
        title={t('chat.roleUnavailable', 'No roles configured. Set them in Settings → Engine.')}
      >
        <span className="h-1.5 w-1.5 rounded-full bg-muted-foreground/40 shrink-0" />
        <KeyRound className="h-3 w-3 shrink-0" />
        <span className="truncate font-medium">{t('chat.roleDefault', 'Default model')}</span>
      </div>
    )
  }

  const isConfigured = currentProvider?.configured ?? false
  const dotClass = isConfigured ? 'bg-success' : 'bg-warning'

  return (
    <Popover>
      <PopoverTrigger asChild>
        <button
          type="button"
          className={cn(
            'inline-flex items-center gap-1.5 h-7 max-w-[220px] truncate rounded-md border border-input bg-background px-2 text-2xs text-foreground',
            'focus:outline-none focus:ring-1 focus:ring-ring focus:ring-offset-0',
            'hover:bg-accent/40 transition-colors',
          )}
          title={t('chat.roleHint', 'Route this message to a specific model via an SDK role')}
          aria-label={t('chat.roleSelect', 'Select a role')}
        >
          <span className={cn('h-1.5 w-1.5 rounded-full shrink-0', dotClass)} />
          <span className="truncate font-medium">
            {currentEntry?.name ?? t('chat.roleDefault', 'Default model')}
          </span>
          {currentShortModel && (
            <span className="ml-1 text-2xs text-muted-foreground truncate font-mono">
              {currentShortModel}
            </span>
          )}
          <span className="ml-auto text-2xs text-muted-foreground/60 shrink-0">▾</span>
        </button>
      </PopoverTrigger>
      <PopoverContent align="start" side="top" className="w-72 p-1.5 text-xs">
        <p className="px-2 py-1 text-2xs uppercase tracking-wider text-muted-foreground font-semibold">
          {t('chat.routing', 'Routing')}
        </p>
        <RoleRow
          label={t('chat.roleDefault', 'Default model')}
          subtitle={t('chat.routeViaConfig', 'System default')}
          configured
          selected={!activeRole}
          onClick={() => onChange(null)}
        />
        <p className="px-2 pt-2 pb-1 text-2xs uppercase tracking-wider text-muted-foreground font-semibold border-t border-border mt-1">
          {t('chat.roles', 'Roles')}
        </p>
        {roles.map((r) => {
          const providerId = r.model.includes('/') ? r.model.split('/')[0] : null
          const provider = providerId ? providerMap.get(providerId) : undefined
          const configured = provider?.configured ?? false
          const shortModel = r.model.includes('/') ? (r.model.split('/').pop() ?? r.model) : r.model
          return (
            <RoleRow
              key={r.name}
              label={r.name}
              subtitle={shortModel}
              configured={configured}
              selected={activeRole === r.name}
              onClick={() => onChange(r.name)}
            />
          )
        })}
      </PopoverContent>
    </Popover>
  )
}

interface RoleRowProps {
  label: string
  subtitle: string
  configured: boolean
  selected: boolean
  onClick: () => void
}

function RoleRow({ label, subtitle, configured, selected, onClick }: RoleRowProps) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        'flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-xs transition-colors',
        'focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring',
        selected ? 'bg-accent text-accent-foreground' : 'hover:bg-accent/50',
      )}
    >
      <span
        className={cn(
          'h-1.5 w-1.5 rounded-full shrink-0',
          configured ? 'bg-success' : 'bg-warning',
        )}
      />
      {!configured && <KeyRound className="h-3 w-3 shrink-0 text-warning" aria-hidden="true" />}
      <span className="truncate font-medium min-w-0">{label}</span>
      <span className="ml-auto text-2xs text-muted-foreground truncate font-mono shrink-0 max-w-[40%]">
        {subtitle}
      </span>
      {selected && <Check className="h-3 w-3 shrink-0 text-primary" aria-hidden="true" />}
    </button>
  )
}
