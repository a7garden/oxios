import { Eye, EyeOff, Key, Shield, Terminal, AlertCircle } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import type { ApiKeySource } from '@/types/engine'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'

// ─── API key source config ───────────────────────────────────

const SOURCE_CONFIG: Record<ApiKeySource, { labelKey: string; icon: React.ReactNode; color: string }> = {
  env: {
    labelKey: 'engine.apiKeyEnv',
    icon: <Terminal className="h-3.5 w-3.5" />,
    color: 'text-emerald-600 dark:text-emerald-400',
  },
  auth_store: {
    labelKey: 'engine.apiKeyAuthStore',
    icon: <Shield className="h-3.5 w-3.5" />,
    color: 'text-blue-600 dark:text-blue-400',
  },
  config: {
    labelKey: 'engine.apiKeyConfig',
    icon: <Key className="h-3.5 w-3.5" />,
    color: 'text-amber-600 dark:text-amber-400',
  },
  none: {
    labelKey: 'engine.apiKeyNone',
    icon: <AlertCircle className="h-3.5 w-3.5" />,
    color: 'text-muted-foreground',
  },
}

// ─── Component ───────────────────────────────────────────────

interface ApiKeyInputProps {
  /** Whether an API key is currently configured */
  hasKey: boolean
  /** Source of the existing key */
  source?: ApiKeySource
  /** Provider name for display */
  providerName: string
  /** Called when user submits a new key */
  onSubmit: (apiKey: string) => void
  /** Whether the mutation is pending */
  isPending?: boolean
  className?: string
}

export function ApiKeyInput({
  hasKey,
  source = 'none',
  providerName,
  onSubmit,
  isPending,
  className,
}: ApiKeyInputProps) {
  const { t } = useTranslation()
  const [inputValue, setInputValue] = useState('')
  const [visible, setVisible] = useState(false)

  const sourceInfo = SOURCE_CONFIG[source]

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (inputValue.trim()) {
      onSubmit(inputValue.trim())
      setInputValue('')
    }
  }

  return (
    <div className={cn('space-y-3', className)}>
      {/* Current status */}
      <div className="flex items-center gap-2 text-sm">
        <span className={cn('flex items-center gap-1.5', sourceInfo.color)}>
          {sourceInfo.icon}
          <span>{t(sourceInfo.labelKey)}</span>
        </span>
      </div>

      {/* Input form */}
      <form onSubmit={handleSubmit} className="flex items-center gap-2">
        <div className="relative flex-1">
          <Input
            type={visible ? 'text' : 'password'}
            value={inputValue}
            onChange={(e) => setInputValue(e.target.value)}
            placeholder={hasKey ? t('engine.apiKeyOverrideHint') : `${providerName} API key를 입력하세요`}
            className="pr-9 font-mono text-sm"
          />
          <button
            type="button"
            onClick={() => setVisible(!visible)}
            className="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
          >
            {visible ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
          </button>
        </div>
        <Button type="submit" size="sm" disabled={!inputValue.trim() || isPending}>
          {isPending ? t('engine.apiKeySaving') : hasKey ? t('engine.apiKeyUpdate') : t('engine.apiKeySetNew')}
        </Button>
      </form>

      {/* Hint */}
      {hasKey && (
        <p className="text-xs text-muted-foreground">
          {t('engine.apiKeyOverrideHint')}
        </p>
      )}
    </div>
  )
}
