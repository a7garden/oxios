import { Select } from '@/components/ui/select'
import { Input } from '@/components/ui/input'
import { Separator } from '@/components/ui/separator'
import { Label } from '@/components/ui/label'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'

// ─── Provider-specific option schemas ────────────────────────

interface OptionDef {
  key: string
  labelKey: string
  descriptionKey: string
  type: 'select' | 'number'
  options?: { value: string; labelKey: string }[]
  placeholderKey?: string
}

const PROVIDER_OPTION_SCHEMAS: Record<string, OptionDef[]> = {
  anthropic: [
    {
      key: 'thinking_type',
      labelKey: 'engine.thinkingType',
      descriptionKey: 'engine.thinkingTypeDescription',
      type: 'select',
      options: [
        { value: 'enabled', labelKey: 'engine.enabled' },
        { value: 'disabled', labelKey: 'engine.disabled' },
      ],
    },
    {
      key: 'thinking_budget_tokens',
      labelKey: 'engine.thinkingBudget',
      descriptionKey: 'engine.thinkingBudgetDescription',
      type: 'number',
      placeholderKey: 'engine.thinkingBudgetPlaceholder',
    },
  ],
  openai: [
    {
      key: 'reasoning_effort',
      labelKey: 'engine.reasoningEffort',
      descriptionKey: 'engine.reasoningEffortDescription',
      type: 'select',
      options: [
        { value: 'low', labelKey: 'engine.low' },
        { value: 'medium', labelKey: 'engine.medium' },
        { value: 'high', labelKey: 'engine.high' },
      ],
    },
    {
      key: 'text_verbosity',
      labelKey: 'engine.textVerbosity',
      descriptionKey: 'engine.textVerbosityDescription',
      type: 'select',
      options: [
        { value: 'low', labelKey: 'engine.verbosityLow' },
        { value: 'medium', labelKey: 'engine.verbosityMedium' },
        { value: 'high', labelKey: 'engine.verbosityHigh' },
      ],
    },
  ],
  google: [
    {
      key: 'thinking_level',
      labelKey: 'engine.thinkingLevel',
      descriptionKey: 'engine.thinkingLevelDescription',
      type: 'select',
      options: [
        { value: 'none', labelKey: 'engine.thinkingNone' },
        { value: 'light', labelKey: 'engine.thinkingLight' },
        { value: 'medium', labelKey: 'engine.thinkingMedium' },
        { value: 'heavy', labelKey: 'engine.thinkingHeavy' },
      ],
    },
    {
      key: 'thinking_budget',
      labelKey: 'engine.thinkingBudget',
      descriptionKey: 'engine.thinkingBudgetDescription',
      type: 'number',
      placeholderKey: 'engine.thinkingBudgetPlaceholder',
    },
  ],
}

// ─── Component ───────────────────────────────────────────────

interface ProviderOptionsProps {
  provider: string
  /** Current option values */
  values: Record<string, unknown>
  /** Called when an option changes */
  onChange: (key: string, value: string | number) => void
  className?: string
}

export function ProviderOptions({ provider, values, onChange, className }: ProviderOptionsProps) {
  const { t } = useTranslation()
  const schema = PROVIDER_OPTION_SCHEMAS[provider]

  if (!schema || schema.length === 0) {
    return (
      <div className={className}>
        <p className="text-sm text-muted-foreground">
          {t('engine.noAdvancedOptionsFor', { provider })}
        </p>
      </div>
    )
  }

  return (
    <div className={className}>
      <div className="space-y-4">
        {schema.map((opt, i) => (
          <div key={opt.key}>
            {i > 0 && <Separator className="mb-4" />}
            <div className="flex items-start justify-between gap-6">
              <div className="flex-1 min-w-0 pt-0.5">
                <Label className="text-sm font-medium">{t(opt.labelKey)}</Label>
                <p className="text-xs text-muted-foreground mt-0.5">{t(opt.descriptionKey)}</p>
              </div>
              <div className="shrink-0 w-56">
                <OptionControl
                  option={opt}
                  value={values[opt.key]}
                  onChange={(val) => onChange(opt.key, val)}
                />
              </div>
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}

// ─── Option control ──────────────────────────────────────────

function OptionControl({
  option,
  value,
  onChange,
}: {
  option: OptionDef
  value: unknown
  onChange: (val: string | number) => void
}) {
  const { t } = useTranslation()

  if (option.type === 'select' && option.options) {
    return (
      <Select
        value={String(value ?? '')}
        onValueChange={(val) => onChange(val)}
        options={option.options.map((o) => ({ value: o.value, label: t(o.labelKey) }))}
        placeholder={option.placeholderKey ? t(option.placeholderKey) : t('common.selectPlaceholder')}
      />
    )
  }

  // Number input
  return (
    <Input
      type="number"
      value={value !== undefined && value !== null ? String(value) : ''}
      onChange={(e) => {
        const num = Number(e.target.value)
        if (!isNaN(num) && e.target.value !== '') {
          onChange(num)
        }
      }}
      placeholder={option.placeholderKey ? t(option.placeholderKey) : undefined}
    />
  )
}

// ─── Wrapper with local state management ─────────────────────

interface ProviderOptionsPanelProps {
  provider: string
  /** Initial option values */
  initialValues?: Record<string, unknown>
  /** Called when user wants to save options */
  onSave: (options: Record<string, unknown>) => void
  isPending?: boolean
  className?: string
}

/**
 * Full provider options panel with local state and save button.
 * Handles the edit → save lifecycle internally.
 */
export function ProviderOptionsPanel({
  provider,
  initialValues = {},
  onSave,
  isPending,
  className,
}: ProviderOptionsPanelProps) {
  const { t } = useTranslation()
  const [localValues, setLocalValues] = useState<Record<string, unknown>>(initialValues)

  // Reset when provider changes
  useEffect(() => {
    setLocalValues(initialValues)
  }, [provider, initialValues])

  const handleChange = (key: string, value: string | number) => {
    setLocalValues((prev) => ({ ...prev, [key]: value }))
  }

  const handleSave = () => {
    onSave(localValues)
  }

  return (
    <div className={className}>
      <ProviderOptions provider={provider} values={localValues} onChange={handleChange} />
      <div className="mt-4 flex justify-end">
        <button
          type="button"
          onClick={handleSave}
          disabled={isPending}
          className="inline-flex items-center justify-center rounded-md bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground shadow hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {isPending ? t('engine.saving') : t('engine.saveOptions')}
        </button>
      </div>
    </div>
  )
}