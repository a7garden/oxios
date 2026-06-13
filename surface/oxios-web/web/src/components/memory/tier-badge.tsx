import { useTranslation } from 'react-i18next'
import { Badge } from '@/components/ui/badge'

const tierColors: Record<string, string> = {
  hot: 'bg-error-subtle text-error',
  warm: 'bg-warning-subtle text-warning',
  cold: 'bg-info-subtle text-info',
}

export function TierBadge({ tier }: { tier: string }) {
  const { t } = useTranslation()
  return (
    <Badge variant="outline" className={tierColors[tier] ?? ''}>
      {t(`memory.${tier}`, tier)}
    </Badge>
  )
}
