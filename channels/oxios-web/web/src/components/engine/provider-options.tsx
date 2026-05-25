import { Select } from '@/components/ui/select'
import { Input } from '@/components/ui/input'
import { Separator } from '@/components/ui/separator'
import { Label } from '@/components/ui/label'
import { useEffect, useState } from 'react'

// ─── Provider-specific option schemas ────────────────────────

interface OptionDef {
  key: string
  label: string
  description: string
  type: 'select' | 'number'
  options?: { value: string; label: string }[]
  placeholder?: string
}

const PROVIDER_OPTION_SCHEMAS: Record<string, OptionDef[]> = {
  anthropic: [
    {
      key: 'thinking_type',
      label: 'Thinking Type',
      description: 'Enable extended thinking for complex reasoning tasks',
      type: 'select',
      options: [
        { value: 'enabled', label: 'Enabled' },
        { value: 'disabled', label: 'Disabled' },
      ],
    },
    {
      key: 'thinking_budget_tokens',
      label: 'Thinking Budget (tokens)',
      description: 'Maximum tokens for thinking output (0 = unlimited)',
      type: 'number',
      placeholder: '10000',
    },
  ],
  openai: [
    {
      key: 'reasoning_effort',
      label: 'Reasoning Effort',
      description: 'Controls how much reasoning the model performs',
      type: 'select',
      options: [
        { value: 'low', label: 'Low' },
        { value: 'medium', label: 'Medium' },
        { value: 'high', label: 'High' },
      ],
    },
    {
      key: 'text_verbosity',
      label: 'Text Verbosity',
      description: 'Controls output length and detail level',
      type: 'select',
      options: [
        { value: 'low', label: 'Low (concise)' },
        { value: 'medium', label: 'Medium' },
        { value: 'high', label: 'High (detailed)' },
      ],
    },
  ],
  google: [
    {
      key: 'thinking_level',
      label: 'Thinking Level',
      description: 'Depth of thinking for reasoning models',
      type: 'select',
      options: [
        { value: 'none', label: 'None' },
        { value: 'light', label: 'Light' },
        { value: 'medium', label: 'Medium' },
        { value: 'heavy', label: 'Heavy' },
      ],
    },
    {
      key: 'thinking_budget',
      label: 'Thinking Budget (tokens)',
      description: 'Maximum tokens for thinking (0 = unlimited)',
      type: 'number',
      placeholder: '8192',
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
  const schema = PROVIDER_OPTION_SCHEMAS[provider]

  if (!schema || schema.length === 0) {
    return (
      <div className={className}>
        <p className="text-sm text-muted-foreground">
          No advanced options available for {provider}.
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
                <Label className="text-sm font-medium">{opt.label}</Label>
                <p className="text-xs text-muted-foreground mt-0.5">{opt.description}</p>
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
  if (option.type === 'select' && option.options) {
    return (
      <Select
        value={String(value ?? '')}
        onValueChange={(val) => onChange(val)}
        options={option.options}
        placeholder="Select..."
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
      placeholder={option.placeholder}
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
          {isPending ? 'Saving...' : 'Save Options'}
        </button>
      </div>
    </div>
  )
}
