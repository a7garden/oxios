import { useTranslation } from 'react-i18next'
import { useToolCatalog } from '@/hooks/use-tool-catalog'
import { ExecAllowlistEditor } from './exec-allowlist-editor'

interface AllowedToolsPickerProps {
  value: string[]
  onChange: (next: string[]) => void
  disabled?: boolean
}

/**
 * Tool multi-select picker. Fetches the tool catalog from the backend
 * and renders an ExecAllowlistEditor with suggestions.
 */
export function AllowedToolsPicker({ value, onChange, disabled }: AllowedToolsPickerProps) {
  const { t } = useTranslation()
  const { data: catalog } = useToolCatalog()

  const suggestions = catalog?.map((tool) => ({
    value: tool.name,
    label: tool.name,
    group: tool.category,
  }))

  return (
    <ExecAllowlistEditor
      value={value}
      onChange={onChange}
      disabled={disabled}
      suggestions={suggestions}
    />
  )
}
