import type { ReactNode } from 'react'
import { cn } from '@/lib/utils'

interface PageHeaderProps {
  /** 페이지 제목. text-2xl font-bold. */
  title: ReactNode
  /** 부제. text-sm text-muted-foreground (항상 명시 크기). */
  subtitle?: ReactNode
  /** 우측 액션 슬롯. RefreshButton · 생성 Button 등. gap-2 정렬. */
  actions?: ReactNode
  /** 제목 행 옆 메타(버전 뱃지 · 카운트 · RFC 태그). 드물게 사용. */
  titleMeta?: ReactNode
  className?: string
}

/**
 * PageHeader — Console 탭 헤더의 단일 진실 원천.
 *
 * 감사(2026-07-18, docs/designs/2026-07-18-ui-consistency-redesign-design.md)
 * P1: 제목 굵기·정렬·부제 크기·앞아이콘·액션갭 5축 분화를 이 컴포넌트로 흡수.
 * 모든 Console 탭은 `<PageHeader>`를 사용한다.
 *
 * 정규형:
 *   h1 = text-2xl font-bold (Dashboard font-semibold tracking-tight 제거 통일)
 *   부제 = text-sm text-muted-foreground (크기 명시)
 *   정렬 = items-center (items-end 금지)
 *   actions = gap-2
 *
 * Chat(/chat) · Knowledge(/knowledge)는 의도적으로 미사용 — 각기 중앙 정렬
 * 채팅 표면·브레드크럼 헤더 패러다임. Settings(/settings)는 3-zone SettingsShell.
 */
export function PageHeader({ title, subtitle, actions, titleMeta, className }: PageHeaderProps) {
  return (
    <div className={cn('flex items-center justify-between gap-4', className)}>
      <div className="min-w-0">
        <div className="flex items-center gap-2 min-w-0">
          <h1 className="text-2xl font-bold truncate">{title}</h1>
          {titleMeta}
        </div>
        {subtitle && <p className="text-sm text-muted-foreground mt-0.5 truncate">{subtitle}</p>}
      </div>
      {actions && <div className="flex items-center gap-2 shrink-0">{actions}</div>}
    </div>
  )
}
