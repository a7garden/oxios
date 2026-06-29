import { CircleX, Plus, Users } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { ModelSelect } from '@/components/engine/model-select'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Separator } from '@/components/ui/separator'
import { useModels, useRoles, useSetRoles } from '@/hooks/use-engine'
import { cn } from '@/lib/utils'

// ─── Translation keys ─────────────────────────────────────────
const tKeys = {
  rolesTitle: 'settings.routing.roles.title',
  rolesDesc: 'settings.routing.roles.desc',
  rolesEmpty: 'settings.routing.roles.empty',
  addRole: 'settings.routing.roles.addRole',
  roleNamePlaceholder: 'settings.routing.roles.namePlaceholder',
  selectModelPlaceholder: 'settings.routing.roles.selectModelPlaceholder',
  duplicateName: 'settings.routing.roles.duplicateName',
  emptyName: 'settings.routing.roles.emptyName',
  noModel: 'settings.routing.roles.noModel',
  failedSave: 'settings.routing.roles.failedSave',
} as const

// Validation rules: name is the role id surfaced in chat; must be a
// non-empty slug, unique within the table. Reserved name "default" is
// reserved for the "no role" hint (clears `activeRole` in the store).
const RESERVED_ROLE_NAMES = new Set(['default'])

/** Normalize a role name: trim, lower-case, replace whitespace with `-`. */
function normalizeRoleName(raw: string): string {
  return raw.trim().toLowerCase().replace(/\s+/g, '-')
}

// ─── RoleSection ──────────────────────────────────────────────

/**
 * RoleSection — RFC-032 role editor for the Engine panel.
 *
 * Roles are named aliases that route a chat message to a specific
 * model without changing the global default. The store persists
 * `activeRole`; chat-input's `RolePill` surfaces this for one-tap
 * routing.
 *
 * Persisted via PUT /api/engine/roles (useSetRoles). Each mutation is
 * a full-table PUT — small N, no patch endpoint needed.
 */
export function RoleSection() {
  const { t } = useTranslation()
  const { data: rolesData, isLoading } = useRoles()
  const { data: models = [] } = useModels(null)
  const setRoles = useSetRoles()

  const roles: Array<{ name: string; model: string }> = Object.entries(
    rolesData?.roles ?? {},
  ).map(([name, model]) => ({ name, model }))

  const [newName, setNewName] = useState('')
  const [newModel, setNewModel] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)

  const validate = (name: string, model: string | null): string | null => {
    const normalized = normalizeRoleName(name)
    if (!normalized) return t(tKeys.emptyName)
    if (RESERVED_ROLE_NAMES.has(normalized)) return t(tKeys.emptyName)
    if (roles.some((r) => r.name === normalized)) return t(tKeys.duplicateName, { name: normalized })
    if (!model) return t(tKeys.noModel)
    return null
  }

  const handleAdd = () => {
    setError(null)
    const normalized = normalizeRoleName(newName)
    const err = validate(newName, newModel)
    if (err) {
      setError(err)
      return
    }
    const next = Object.fromEntries([
      ...roles.map((r) => [r.name, r.model] as const),
      [normalized, newModel!] as const,
    ])
    setRoles.mutate(next, {
      onSuccess: () => {
        setNewName('')
        setNewModel(null)
      },
      onError: () => setError(t(tKeys.failedSave)),
    })
  }

  const handleRemove = (name: string) => {
    setError(null)
    const next = Object.fromEntries(
      roles.filter((r) => r.name !== name).map((r) => [r.name, r.model] as const),
    )
    setRoles.mutate(next, {
      onError: () => setError(t(tKeys.failedSave)),
    })
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      e.preventDefault()
      handleAdd()
    }
  }

  if (isLoading) return null

  return (
    <div className="space-y-6">
      <Separator />

      <div className="space-y-4">
        <div className="flex items-center gap-2">
          <Users className="h-4 w-4 text-muted-foreground" />
          <h3 className="text-lg font-semibold">{t(tKeys.rolesTitle)}</h3>
        </div>
        <p className="text-sm text-muted-foreground">{t(tKeys.rolesDesc)}</p>

        {roles.length === 0 ? (
          <p className="text-xs text-muted-foreground/70 italic">
            {t(tKeys.rolesEmpty)}
          </p>
        ) : (
          <div className="space-y-2">
            {roles.map((r) => (
              <RoleRow
                key={r.name}
                name={r.name}
                model={r.model}
                onRemove={() => handleRemove(r.name)}
                disabled={setRoles.isPending}
              />
            ))}
          </div>
        )}

        {/* Add row: name + model + add button */}
        <div className="flex flex-col sm:flex-row items-stretch gap-2 pt-1">
          <div className="flex-1 min-w-0">
            <Input
              value={newName}
              onChange={(e) => {
                setNewName(e.target.value)
                if (error) setError(null)
              }}
              onKeyDown={handleKeyDown}
              placeholder={t(tKeys.roleNamePlaceholder)}
              className="h-9 text-sm"
              disabled={setRoles.isPending}
              maxLength={48}
            />
          </div>
          <div className="flex-1 min-w-0">
            <ModelSelect
              models={models}
              value={newModel}
              onValueChange={(id) => {
                setNewModel(id)
                if (error) setError(null)
              }}
              className="h-9"
            />
          </div>
          <Button
            variant="outline"
            size="sm"
            className="h-9 shrink-0"
            onClick={handleAdd}
            disabled={setRoles.isPending || !newName.trim() || !newModel}
          >
            <Plus className="h-3.5 w-3.5 mr-1" />
            {t(tKeys.addRole)}
          </Button>
        </div>

        {error && (
          <p className="text-xs text-destructive" role="alert">
            {error}
          </p>
        )}
      </div>
    </div>
  )
}

// ─── RoleRow ──────────────────────────────────────────────────

function RoleRow({
  name,
  model,
  onRemove,
  disabled,
}: {
  name: string
  model: string
  onRemove: () => void
  disabled?: boolean
}) {
  const { t } = useTranslation()
  // `provider/model` → show "model" muted and "provider" emphasized.
  const [provider, ...rest] = model.split('/')
  const shortModel = rest.join('/') || model

  return (
    <div
      className={cn(
        'flex items-center gap-2 rounded-md border bg-muted/20 px-3 py-2',
        disabled && 'opacity-60',
      )}
    >
      <div className="flex-1 min-w-0 flex items-center gap-2">
        <span className="font-medium text-sm truncate" title={name}>
          {name}
        </span>
        <span className="text-muted-foreground/50 text-xs shrink-0">→</span>
        <span className="text-xs text-muted-foreground truncate font-mono" title={model}>
          {provider && provider !== shortModel ? (
            <>
              <span className="text-foreground/80 font-sans">{provider}</span>
              <span className="text-muted-foreground/60">/</span>
              {shortModel}
            </>
          ) : (
            model
          )}
        </span>
      </div>
      <Button
        variant="ghost"
        size="sm"
        className="h-7 w-7 p-0 shrink-0"
        onClick={onRemove}
        disabled={disabled}
        title={t('common.delete', 'Delete')}
        aria-label={`${t('common.delete', 'Delete')} ${name}`}
      >
        <CircleX className="h-4 w-4 text-muted-foreground" />
      </Button>
    </div>
  )
}
