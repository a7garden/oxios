import { useTranslation } from 'react-i18next'
import { Input } from '@/components/ui/input'
import { Switch } from '@/components/ui/switch'
import { Select } from '@/components/ui/select'
import { Textarea } from '@/components/ui/textarea'
import { RestartBadge } from './restart-badge'
import { ExecAllowlistEditor } from './exec-allowlist-editor'
import type { SettingsFieldDef } from './field-defs'

interface FieldRowProps {
  sectionKey: string
  field: SettingsFieldDef
  value: string | boolean | string[] | number | undefined
  onChange: (val: string | boolean | string[]) => void
}

/**
 * Single labelled form row. Renders the appropriate control for
 * `field.type` and shows a hot-reload / restart badge next to the label.
 */
export function FieldRow({ sectionKey, field, value, onChange }: FieldRowProps) {
  const { t } = useTranslation()
  const id = `${sectionKey}-${field.key.replace(/\./g, '-')}`

  return (
    <div className="flex items-start justify-between gap-4 sm:gap-6">
      <div className="flex-1 min-w-0 pt-0.5">
        <div className="flex items-center gap-2 flex-wrap">
          <label htmlFor={id} className="text-sm font-medium">
            {t(field.labelKey)}
          </label>
          <RestartBadge hotReload={field.hotReload} scope={field.restartScope} />
        </div>
        <p className="text-xs text-muted-foreground mt-0.5">{t(field.descriptionKey)}</p>
      </div>

      <div className="shrink-0 w-40 sm:w-56">
        <FieldControl id={id} field={field} value={value} onChange={onChange} />
      </div>
    </div>
  )
}

function FieldControl({ id, field, value, onChange }: { id: string; field: SettingsFieldDef; value: unknown; onChange: (v: string | boolean | string[]) => void }) {
  const { t } = useTranslation()
  switch (field.type) {
    case 'toggle': {
      return (
        <div className="flex items-center justify-end gap-2">
          <span className="text-xs text-muted-foreground">
            {value ? t('common.on') : t('common.off')}
          </span>
          <Switch
            id={id}
            checked={Boolean(value)}
            onCheckedChange={(checked) => onChange(checked)}
          />
        </div>
      )
    }
    case 'select': {
      return (
        <Select
          value={String(value ?? '')}
          onValueChange={(v) => onChange(v)}
          placeholder={t(field.labelKey)}
          options={field.options?.map((opt) => ({
            label: t(opt.labelKey),
            value: opt.value,
          })) ?? []}
          className="w-full"
        />
      )
    }
    case 'number': {
      return (
        <Input
          id={id}
          type="number"
          value={String(value ?? '')}
          onChange={(e) => onChange(e.target.value)}
          placeholder={field.placeholder}
        />
      )
    }
    case 'password': {
      return (
        <Input
          id={id}
          type="password"
          value={String(value ?? '')}
          onChange={(e) => onChange(e.target.value)}
          placeholder={field.placeholder}
        />
      )
    }
    case 'multiline': {
      return (
        <Textarea
          id={id}
          value={String(value ?? '')}
          onChange={(e) => onChange(e.target.value)}
          placeholder={field.placeholder}
        />
      )
    }
    case 'csv': {
      // Comma-separated list. Convert on every change.
      const stringified = Array.isArray(value) ? value.join(', ') : String(value ?? '')
      return (
        <Input
          id={id}
          type="text"
          value={stringified}
          onChange={(e) => onChange(e.target.value)}
          placeholder={field.placeholder}
        />
      )
    }
    case 'numbers': {
      // Multi-line number list (one per line). Stored as string, parsed on save.
      const stringified = Array.isArray(value) ? value.join('\n') : String(value ?? '')
      return (
        <Textarea
          id={id}
          value={stringified}
          onChange={(e) => onChange(e.target.value)}
          placeholder={field.placeholder}
          className="font-mono text-xs"
          rows={3}
        />
      )
    }
    case 'tags': {
      const arr = Array.isArray(value) ? (value as string[]) : []
      return <ExecAllowlistEditor value={arr} onChange={(next) => onChange(next)} />
    }
    case 'text':
    default: {
      return (
        <Input
          id={id}
          type="text"
          value={String(value ?? '')}
          onChange={(e) => onChange(e.target.value)}
          placeholder={field.placeholder}
        />
      )
    }
  }
}
