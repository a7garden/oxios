import { useTranslation } from 'react-i18next'
import { Badge } from '@/components/ui/badge'

export function TypeBadge({ type }: { type: string }) {
  const { t } = useTranslation()
  return (
    <Badge variant="secondary" className="text-xs">
      {t(`memory.${type}`, type)}
    </Badge>
  )
}
