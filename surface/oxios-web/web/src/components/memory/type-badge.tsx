import { Badge } from '@/components/ui/badge'
import { useTranslation } from 'react-i18next'

export function TypeBadge({ type }: { type: string }) {
  const { t } = useTranslation()
  return (
    <Badge variant="secondary" className="text-xs">
      {t(`memory.${type}`, type)}
    </Badge>
  )
}
