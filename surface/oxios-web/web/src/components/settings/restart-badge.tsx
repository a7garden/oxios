import { AlertTriangle, Check } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { cn } from '@/lib/utils'

interface RestartBadgeProps {
  /** Whether the field is hot-reloadable (true = immediate, false = restart). */
  hotReload: boolean
  /** Optional restart scope for the tooltip. */
  scope?: 'kernel' | 'gateway' | 'logging' | 'memory' | 'engine' | 'audit'
  /** Show inline (next to the field) vs compact. */
  size?: 'sm' | 'md'
  className?: string
}

/**
 * Small badge that signals whether a field is hot-reloadable or needs a
 * restart. Used next to field labels in the Settings UI.
 */
export function RestartBadge({ hotReload, scope, size = 'sm', className }: RestartBadgeProps) {
  const { t } = useTranslation()
  const label = hotReload ? t('settings.hotReload') : t('settings.requiresRestart')
  const tooltipContent = hotReload
    ? t('settings.hotReloadTooltip')
    : t('settings.requiresRestartTooltip', {
        scope: scope ? t(`settings.scope_${scope}`) : t('settings.scope_daemon'),
      })

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <span
        className={cn(
          'inline-flex items-center gap-1 rounded-full border font-medium select-none whitespace-nowrap',
          size === 'sm' ? 'px-1.5 py-0.5 text-2xs' : 'px-2 py-0.5 text-xs',
          hotReload
            ? 'border-success-subtle bg-success-subtle text-success'
            : 'border-warning-subtle bg-warning-subtle text-warning',
          className,
        )}
        data-testid={hotReload ? 'hot-reload-badge' : 'restart-badge'}
      >
        {hotReload ? (
          <Check className={size === 'sm' ? 'h-2.5 w-2.5' : 'h-3 w-3'} />
        ) : (
          <AlertTriangle className={size === 'sm' ? 'h-2.5 w-2.5' : 'h-3 w-3'} />
        )}
        <span>{label}</span>
      </span>
      </TooltipTrigger>
      <TooltipContent side="top">{tooltipContent}</TooltipContent>
    </Tooltip>
  )
}
