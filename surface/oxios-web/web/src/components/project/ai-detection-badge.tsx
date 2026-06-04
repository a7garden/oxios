import { Check } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import type { Project } from '@/types'
import { Button } from '@/components/ui/button'
import { getProjectIcon } from './project-icon'

interface AiDetectionBadgeProps {
  project: Project
  onApply: () => void
  onDismiss: () => void
}

export function AiDetectionBadge({ project, onApply, onDismiss }: AiDetectionBadgeProps) {
  const { t } = useTranslation()

  return (
    <div className="flex items-center gap-2 px-3 py-2 rounded-lg border bg-amber-50 dark:bg-amber-950/50 border-amber-300 dark:border-amber-700 text-amber-800 dark:text-amber-300 text-xs animate-in slide-in-from-top-2 fade-in duration-200">
      <span className="shrink-0">{getProjectIcon(project.emoji)}</span>
      <span className="font-medium">{project.name}</span>
      <span className="text-amber-600 text-[10px] shrink-0">Detected</span>
      <div className="ml-auto flex items-center gap-1 shrink-0">
        <Button
          size="sm"
          variant="ghost"
          className="h-5 px-2 text-[10px] text-emerald-700 hover:text-emerald-800 hover:bg-emerald-100"
          onClick={onApply}
        >
          {t('projects.apply', 'Apply')}
        </Button>
        <Button
          size="sm"
          variant="ghost"
          className="h-5 px-2 text-[10px] text-amber-700 hover:text-amber-800 hover:bg-amber-100"
          onClick={onDismiss}
        >
          <Check className="h-3 w-3" />
        </Button>
      </div>
    </div>
  )
}