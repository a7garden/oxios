import { useTranslation } from 'react-i18next'
import { Input } from '@/components/ui/input'
import { Select } from '@/components/ui/select'
import { Switch } from '@/components/ui/switch'
import { Textarea } from '@/components/ui/textarea'
import { cn } from '@/lib/utils'
import { ExecAllowlistEditor } from './exec-allowlist-editor'
import type { SettingsFieldDef } from './field-defs'

interface FieldRowProps {
  /** Section key, e.g. `exec` or `memory`. */
  sectionKey: string
  /** The field definition. */
  field: SettingsFieldDef
  /** Current form value. */
  value: string | boolean | string[] | number | undefined
  /** Change handler. */
  onChange: (val: string | boolean | string[]) => void
  /**
   * Whether this row has a pending (unsaved) change. Renders a 2px
   * primary accent on the left edge of the row when true.
   */
  modified?: boolean
  /**
   * Manual disable override. When true, the field is disabled
   * regardless of `dependsOn` evaluation. Used by sections with
   * their own form state (e.g. RoutingSection, ProviderOptions).
   */
  disabled?: boolean
  /**
   * Section-level form values. When `field.dependsOn` is set, this
   * is used to evaluate whether the field should be disabled based
   * on the parent field's value.
   */
  sectionValues?: Record<string, unknown>
}

/**
 * Single labelled form row. Renders the appropriate control for
 * `field.type`. Restart-required info is shown only at save time (DiffPreview),
 * not as a per-field badge.
 *
 * Layout is responsive:
 *  - < 768px: stacked (label/description/badge above, control full-width below)
 *  - >= 768px: 2-column (label block left, control right at 320px)
 */
export function FieldRow({
  sectionKey,
  field,
  value,
  onChange,
  modified,
  disabled: disabledProp,
  sectionValues,
}: FieldRowProps) {
  const { t } = useTranslation()
  const id = `${sectionKey}-${field.key.replace(/\./g, '-')}`

  // Evaluate dependsOn: if the parent field's value doesn't match,
  // the field is automatically disabled.
  const dependsOnDisabled =
    field.dependsOn && sectionValues
      ? sectionValues[field.dependsOn.field] !== field.dependsOn.value
      : false
  const isDisabled = disabledProp || dependsOnDisabled

  return (
    <div
      className={cn(
        // Responsive field row (spec §5):
        //   < md    : stacked (label above, control full-width below)
        //   md–lg  : 2-col fluid ratio 40/60 (control scales with width)
        //   lg–xl  : label fluid + control fixed 320px
        //   ≥ xl   : label fluid + control fixed 360px
        'group/field relative flex flex-col gap-3',
        'md:grid md:grid-cols-[2fr_3fr] md:items-start md:gap-x-6',
        'lg:grid-cols-[minmax(0,1fr)_320px] lg:gap-x-8',
        'xl:grid-cols-[minmax(0,1fr)_360px]',
        'rounded-lg px-3 -mx-3 py-3 transition-colors',
        'hover:bg-muted/30',
        modified && 'bg-modified-row-bg hover:bg-modified-row-bg',
      )}
      data-modified={modified ? 'true' : undefined}
    >
      {/* Modified accent bar (left edge) */}
      {modified && (
        <span
          aria-hidden
          className="absolute left-0 top-2 bottom-2 w-0.5 rounded-full bg-modified-accent"
        />
      )}

      {/* Label block */}
      <div className="min-w-0">
        <div className="flex items-center gap-2 flex-wrap">
          <label htmlFor={id} className="text-sm font-medium text-foreground">
            {t(field.labelKey)}
          </label>
        </div>
        <p className="mt-1 text-xs text-muted-foreground leading-relaxed">
          {t(field.descriptionKey)}
        </p>
      </div>

      {/* Control */}
      <div className="w-full md:w-auto">
        <FieldControl
          id={id}
          field={field}
          value={value}
          onChange={onChange}
          disabled={isDisabled}
        />
      </div>
    </div>
  )
}

function FieldControl({
  id,
  field,
  value,
  onChange,
  disabled,
}: {
  id: string
  field: SettingsFieldDef
  value: unknown
  onChange: (v: string | boolean | string[]) => void
  disabled: boolean
}) {
  const { t } = useTranslation()
  switch (field.type) {
    case 'toggle': {
      return (
        <div className="flex items-center justify-end gap-2 md:justify-start">
          <span
            className={`text-xs tabular-nums ${disabled ? 'text-muted-foreground/50' : 'text-muted-foreground'}`}
          >
            {value ? t('common.on') : t('common.off')}
          </span>
          <Switch
            id={id}
            checked={Boolean(value)}
            onCheckedChange={(checked) => onChange(checked)}
            disabled={disabled}
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
          options={
            Array.isArray(field.options)
              ? field.options.map((opt) => ({
                  label: t(opt.labelKey),
                  value: opt.value,
                }))
              : []
          }
          className="w-full"
          disabled={disabled}
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
          className="font-mono"
          disabled={disabled}
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
          className="font-mono"
          disabled={disabled}
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
          className="font-mono text-xs"
          rows={3}
          disabled={disabled}
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
          className="font-mono text-xs"
          disabled={disabled}
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
          disabled={disabled}
        />
      )
    }
    case 'tags': {
      const arr = Array.isArray(value) ? (value as string[]) : []
      return (
        <ExecAllowlistEditor value={arr} onChange={(next) => onChange(next)} disabled={disabled} />
      )
    }
    default: {
      return (
        <Input
          id={id}
          type="text"
          value={String(value ?? '')}
          onChange={(e) => onChange(e.target.value)}
          placeholder={field.placeholder}
          className="font-mono"
          disabled={disabled}
        />
      )
    }
  }
}
