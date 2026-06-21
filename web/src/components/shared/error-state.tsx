import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'

interface ErrorStateProps {
  title?: string
  message?: string
  onRetry?: () => void
  className?: string
}

export function ErrorState({ title, message, onRetry, className }: ErrorStateProps) {
  const { t } = useTranslation()
  const resolvedTitle = title ?? t('common.errorFailedToLoad')
  const resolvedMessage = message ?? t('common.errorSomethingWrong')
  return (
    <div
      className={cn(
        'flex flex-col items-center justify-center py-12 text-center animate-fade-in-up',
        className,
      )}
      role="alert"
    >
      <h3 className="text-lg font-semibold text-destructive">{resolvedTitle}</h3>
      <p className="mt-1 text-sm text-muted-foreground max-w-md">{resolvedMessage}</p>
      {onRetry && (
        <Button variant="default" onClick={onRetry} className="mt-4">
          {t('common.retry')}
        </Button>
      )}
    </div>
  )
}
