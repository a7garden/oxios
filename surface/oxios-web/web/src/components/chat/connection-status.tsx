import { cn } from '@/lib/utils'
import { useTranslation } from 'react-i18next'

interface ConnectionStatusProps {
  connected: boolean
  className?: string
}

export function ConnectionStatus({ connected, className }: ConnectionStatusProps) {
  const { t } = useTranslation()
  return (
    <span className={cn('flex items-center gap-1.5 text-xs', className)}>
      <span
        className={cn(
          'h-2 w-2 rounded-full',
          connected ? 'bg-success' : 'bg-warning animate-pulse',
        )}
      />
      {connected ? t('chat.connected', 'Connected') : t('chat.connecting', 'Connecting...')}
    </span>
  )
}
