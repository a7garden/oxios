import { useTranslation } from 'react-i18next'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { useCostDaily } from '@/hooks/use-costs'
import {
  Bar,
  BarChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from 'recharts'
import { EmptyState } from '@/components/shared/empty-state'

interface Props {
  days?: number
}

export function CostChart({ days = 30 }: Props) {
  const { t } = useTranslation()
  const { data, isLoading } = useCostDaily(days)

  const items = data?.items ?? []

  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base">
          {t('cost.dailySpend')} ({days}d)
        </CardTitle>
      </CardHeader>
      <CardContent>
        {isLoading ? (
          <p className="text-sm text-muted-foreground py-4">{t('common.loading')}</p>
        ) : items.length === 0 ? (
          <EmptyState title={t('cost.noData')} />
        ) : (
          <ResponsiveContainer width="100%" height={240}>
            <BarChart data={items}>
              <CartesianGrid strokeDasharray="3 3" className="stroke-muted" />
              <XAxis
                dataKey="date"
                tick={{ fontSize: 10 }}
                tickFormatter={(v: string) => v.slice(5)}
              />
              <YAxis
                tick={{ fontSize: 10 }}
                tickFormatter={(v: number) => `$${v.toFixed(2)}`}
                width={60}
              />
              <Tooltip
                formatter={(value) => [`$${Number(value).toFixed(4)}`, t('cost.spend')]}
                labelStyle={{ fontSize: 12 }}
              />
              <Bar
                dataKey="cost_usd"
                fill="hsl(var(--primary))"
                radius={[4, 4, 0, 0]}
              />
            </BarChart>
          </ResponsiveContainer>
        )}
      </CardContent>
    </Card>
  )
}
