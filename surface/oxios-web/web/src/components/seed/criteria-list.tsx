import { CheckCircle2, XCircle } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'

export function CriteriaList({
  criteria,
  results,
}: {
  criteria: string[]
  results?: boolean[]
}) {
  const { t } = useTranslation()
  if (!criteria?.length) return null
  return (
    <Card>
      <CardHeader>
        <CardTitle>{t('seeds.acceptanceCriteria')}</CardTitle>
      </CardHeader>
      <CardContent>
        <ul className="space-y-1">
          {criteria.map((c, i) => {
            const passed = results?.[i]
            return (
              <li key={i} className="flex items-start gap-2 text-sm">
                {passed === true ? (
                  <CheckCircle2 className="mt-0.5 h-4 w-4 text-green-500" />
                ) : passed === false ? (
                  <XCircle className="mt-0.5 h-4 w-4 text-red-500" />
                ) : (
                  <span className="mt-0.5 text-muted-foreground">○</span>
                )}
                <span>{c}</span>
              </li>
            )
          })}
        </ul>
      </CardContent>
    </Card>
  )
}
