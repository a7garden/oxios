import { Lock, Shield } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Badge } from '@/components/ui/badge'

const protectionDisplay: Record<string, { icon: React.ReactNode; count: number }> = {
  none: { icon: null, count: 0 },
  low: { icon: <Shield className="h-3 w-3" />, count: 1 },
  medium: { icon: <Shield className="h-3 w-3" />, count: 2 },
  high: { icon: <Shield className="h-3 w-3" />, count: 3 },
  permanent: { icon: <Lock className="h-3 w-3" />, count: 1 },
}

export function ProtectionBadge({ level }: { level: string }) {
  const { t } = useTranslation()
  const display = protectionDisplay[level] ?? protectionDisplay.none!
  return (
    <Badge variant="outline" className="text-xs gap-1">
      {Array.from({ length: display.count }, (_, i) => (
        <span key={i}>{display.icon}</span>
      ))}
      {t(`memory.${level}`, level)}
    </Badge>
  )
}
