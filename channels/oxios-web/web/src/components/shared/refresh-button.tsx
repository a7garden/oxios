import { RefreshCw } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'

interface RefreshButtonProps {
  /** Called when the button is clicked / refresh is triggered. */
  onClick: () => void
  /** Show spinning animation. */
  isFetching?: boolean
  /** Visual variant – "button" (default, for page headers) or "icon" (minimal). */
  variant?: 'button' | 'icon'
  /** Disable the button. */
  disabled?: boolean
}

/**
 * Canonical refresh button used across all pages.
 * Replaces the mix of `<Button variant="outline">` and bare `<button>` patterns.
 */
export function RefreshButton({
  onClick,
  isFetching = false,
  variant = 'button',
  disabled = false,
}: RefreshButtonProps) {
  const { t } = useTranslation()
  if (variant === 'icon') {
    return (
      <button
        type="button"
        onClick={onClick}
        aria-label={t('common.refresh')}
        disabled={disabled}
        className="rounded-md p-2 hover:bg-muted transition-colors"
      >
        <RefreshCw className={`h-4 w-4 ${isFetching ? 'animate-spin' : ''}`} />
      </button>
    )
  }

  return (
    <Button variant="outline" size="sm" onClick={onClick} disabled={disabled}>
      <RefreshCw className={`h-4 w-4 mr-1 ${isFetching ? 'animate-spin' : ''}`} /> {t('common.refresh')}
    </Button>
  )
}
