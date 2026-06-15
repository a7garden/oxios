import { validateCorsOrigin } from '@/lib/cors-validator'
import { ExecAllowlistEditor } from './exec-allowlist-editor'

interface CorsOriginsEditorProps {
  value: string[]
  onChange: (next: string[]) => void
  disabled?: boolean
}

/**
 * CORS origins URL list editor. Validates each URL before adding.
 */
export function CorsOriginsEditor({ value, onChange, disabled }: CorsOriginsEditorProps) {
  return (
    <ExecAllowlistEditor
      value={value}
      onChange={onChange}
      disabled={disabled}
      validate={validateCorsOrigin}
    />
  )
}
