import { CheckCircle2, HelpCircle, XCircle } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Badge } from '@/components/ui/badge'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import type { EvaluationResult } from '@/types/seed'

export function EvaluationCard({ evaluation }: { evaluation?: EvaluationResult }) {
  const { t } = useTranslation()
  if (!evaluation) return null

  const items = [
    {
      label: t('seeds.mechanical'),
      passed: evaluation.mechanical?.passed,
      detail: evaluation.mechanical?.details,
    },
    {
      label: t('seeds.semantic'),
      passed: evaluation.semantic?.passed,
      detail: evaluation.semantic?.details,
    },
    {
      label: t('seeds.consensus'),
      passed: evaluation.consensus?.agreed,
      detail: evaluation.consensus?.details,
    },
  ]

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t('seeds.evaluationResult')}</CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="grid gap-3 md:grid-cols-3">
          {items.map((item) => (
            <div key={item.label} className="flex items-center gap-2 rounded-lg border p-3">
              {item.passed === true ? (
                <CheckCircle2 className="h-5 w-5 text-green-500" />
              ) : item.passed === false ? (
                <XCircle className="h-5 w-5 text-error" />
              ) : (
                <HelpCircle className="h-5 w-5 text-muted-foreground" />
              )}
              <div>
                <p className="text-sm font-medium">{item.label}</p>
                {item.detail && <p className="text-xs text-muted-foreground">{item.detail}</p>}
              </div>
            </div>
          ))}
        </div>
        <div className="flex items-center gap-2">
          <span className="text-sm text-muted-foreground">{t('seeds.score')}:</span>
          <div className="flex items-center gap-2 flex-wrap">
            <div className="h-2 w-32 min-w-[80px] overflow-hidden rounded-full bg-muted">
              <div
                className="h-full rounded-full bg-primary"
                style={{ width: `${(evaluation.score || 0) * 100}%` }}
              />
            </div>
            <Badge variant="outline">{(evaluation.score || 0).toFixed(2)} / 1.0</Badge>
          </div>
        </div>
      </CardContent>
    </Card>
  )
}
