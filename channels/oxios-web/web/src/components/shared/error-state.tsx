import { useTranslation } from 'react-i18next'
import { cn } from '@/lib/utils'

interface ErrorStateProps {
  title?: string
  message?: string
  onRetry?: () => void
  className?: string
}

export function ErrorState({
  title,
  message,
  onRetry,
  className,
}: ErrorStateProps) {
  const { t } = useTranslation()
  const resolvedTitle = title ?? t('common.errorFailedToLoad')
  const resolvedMessage = message ?? t('common.errorSomethingWrong')
  return (
    <div
      className={cn('flex flex-col items-center justify-center py-12 text-center', className)}
      role="alert"
    >
      <h3 className="text-lg font-semibold text-destructive">{resolvedTitle}</h3>
      <p className="mt-1 text-sm text-muted-foreground max-w-md">{resolvedMessage}</p>
      {onRetry && (
        <button
          type="button"
          onClick={onRetry}
          className="mt-4 rounded-md bg-primary px-4 py-2 text-sm text-primary-foreground hover:bg-primary/90"
        >
          {t('common.retry')}
        </button>
      )}
    </div>
  )
}
