import { Badge } from '@/components/ui/badge'
import { useTranslation } from 'react-i18next'

const protectionIcons: Record<string, string> = {
  none: '',
  low: '🛡️',
  medium: '🛡️🛡️',
  high: '🛡️🛡️🛡️',
  permanent: '🔒',
}

export function ProtectionBadge({ level }: { level: string }) {
  const { t } = useTranslation()
  return (
    <Badge variant="outline" className="text-xs gap-1">
      {protectionIcons[level]} {t(`memory.${level}`, level)}
    </Badge>
  )
}
