import { cn } from '@/lib/utils'

interface EmptyStateProps {
  icon?: React.ReactNode
  title: string
  description?: string
  action?: React.ReactNode
  /** 여분 콘텐츠 슬롯(예: 최근 목록). action 아래, 중앙 정렬 컨터이너 안에 렌더. */
  children?: React.ReactNode
  className?: string
  /**
   * 렌더 스케일.
   * - 'page' (기본): 콘솔 탭·목록 빈 상태. py-12, text-lg 제목.
   * - 'compact': 팝오버·드롭다운 내부. py-8, text-xs 제목.
   */
  size?: 'page' | 'compact'
}

export function EmptyState({
  icon,
  title,
  description,
  action,
  children,
  className,
  size = 'page',
}: EmptyStateProps) {
  const compact = size === 'compact'
  return (
    <div
      className={cn(
        'flex flex-col items-center justify-center text-center',
        compact ? 'py-8 px-4 gap-1' : 'py-12 animate-fade-in-up',
        className,
      )}
      role="status"
    >
      {icon && (
        <div
          className={cn(compact ? 'opacity-50' : 'mb-4 text-muted-foreground')}
          aria-hidden="true"
        >
          {icon}
        </div>
      )}
      <h3 className={compact ? 'text-xs font-medium text-foreground/80' : 'text-lg font-semibold'}>
        {title}
      </h3>
      {description && (
        <p
          className={cn(
            'mt-1 text-muted-foreground',
            compact ? 'text-2xs max-w-[18rem] opacity-70' : 'text-sm max-w-md',
          )}
        >
          {description}
        </p>
      )}
      {action && <div className={compact ? 'mt-3' : 'mt-4'}>{action}</div>}
      {children}
    </div>
  )
}
