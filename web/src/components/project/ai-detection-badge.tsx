import { Check } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import type { Project } from '@/types'
import { getProjectIcon } from './project-icon'

interface AiDetectionBadgeProps {
  project: Project
  onApply: () => void
  onDismiss: () => void
}

export function AiDetectionBadge({ project, onApply, onDismiss }: AiDetectionBadgeProps) {
  const { t } = useTranslation()

  return (
    <div className="flex items-center gap-2 px-3 py-2 rounded-lg border bg-warning-subtle border-warning-subtle-border text-warning text-xs animate-in slide-in-from-top-2 fade-in duration-200">
      <span className="shrink-0">{getProjectIcon(project.emoji)}</span>
      <span className="font-medium">{project.name}</span>
      <span className="text-warning/70 text-2xs shrink-0">Detected</span>
      <div className="ml-auto flex items-center gap-1 shrink-0">
        <Button
          size="sm"
          variant="ghost"
          className="h-5 px-2 text-2xs text-success hover:bg-success-subtle"
          onClick={onApply}
        >
          {t('projects.apply')}
        </Button>
        <Button
          size="sm"
          variant="ghost"
          className="h-5 px-2 text-2xs text-warning hover:bg-warning-subtle"
          onClick={onDismiss}
        >
          <Check className="h-3 w-3" />
        </Button>
      </div>
    </div>
  )
}
