import { useTranslation } from 'react-i18next'
import { Badge } from '@/components/ui/badge'

const tierColors: Record<string, string> = {
  hot: 'bg-red-100 text-red-800 dark:bg-red-900/30 dark:text-red-400',
  warm: 'bg-yellow-100 text-yellow-800 dark:bg-yellow-900/30 dark:text-yellow-400',
  cold: 'bg-blue-100 text-blue-800 dark:bg-blue-900/30 dark:text-blue-400',
}

export function TierBadge({ tier }: { tier: string }) {
  const { t } = useTranslation()
  return (
    <Badge variant="outline" className={tierColors[tier] ?? ''}>
      {t(`memory.${tier}`, tier)}
    </Badge>
  )
}
