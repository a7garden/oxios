import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'

interface MetricGaugeCardProps {
  label: string
  /** 0–100 사용률(%). */
  value: number
  className?: string
}

/**
 * MetricGaugeCard — 단일 자원 사용률(0–100%)을 라벨 + 큰 값 + 심각도 게이지 바로 표시.
 *
 * Dashboard StatCard(시계열 KPI + 스파크라인)와 구분되는 "현재 순간 게이지" 패턴의
 * 공유 카드. Resources(CPU/Memory/Disk)의 3-copy 중복을 제거하고 카드 문법을 정규화하기
 * 위해 도입. 심각도 색은 info/warning/error 시맨틱 토큰에서 해석(임계값 75/90).
 */
export function MetricGaugeCard({ label, value, className }: MetricGaugeCardProps) {
  // 0–100 사용률 심각도: info(<75) · warning(75–90) · error(90+).
  // SVG 인라인 style은 var()를 직접 쓸 수 없어 getComputedStyle로 해석.
  const sevToken = value >= 90 ? '--error' : value >= 75 ? '--warning' : '--info'
  const sevColor =
    typeof window === 'undefined'
      ? '#888'
      : getComputedStyle(document.documentElement).getPropertyValue(sevToken).trim() || '#888'

  return (
    <Card className={className}>
      <CardHeader className="pb-2">
        <CardTitle className="text-sm text-muted-foreground">{label}</CardTitle>
      </CardHeader>
      <CardContent>
        <div className="text-2xl font-bold">{value.toFixed(1)}%</div>
        <div className="mt-2 h-2 rounded-full bg-muted overflow-hidden">
          <div
            className="h-full rounded-full transition-all"
            style={{ width: `${value}%`, backgroundColor: sevColor }}
          />
        </div>
      </CardContent>
    </Card>
  )
}
