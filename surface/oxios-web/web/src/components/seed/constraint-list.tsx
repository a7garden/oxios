import { AlertTriangle } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'

export function ConstraintList({ constraints }: { constraints: string[] }) {
  const { t } = useTranslation()
  if (!constraints?.length) return null
  return (
    <Card>
      <CardHeader>
        <CardTitle className="flex items-center gap-2">
          <AlertTriangle className="h-4 w-4" /> {t('seeds.constraints')}
        </CardTitle>
      </CardHeader>
      <CardContent>
        <ul className="space-y-1">
          {constraints.map((c, i) => (
            <li key={i} className="flex items-start gap-2 text-sm">
              <span className="mt-0.5 text-muted-foreground">•</span>
              <span>{c}</span>
            </li>
          ))}
        </ul>
      </CardContent>
    </Card>
  )
}
