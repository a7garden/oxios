import { Settings } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { KnowledgeSettings } from './knowledge-settings'

interface KnowledgeSettingsDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

/**
 * Modal wrapper for knowledge-base configuration.
 *
 * Renders as a centered dialog on desktop and a bottom sheet on mobile
 * (via the Dialog primitive's `mobileSheet` mode), replacing the former
 * full-page `/knowledge/settings` route so the user keeps their editing
 * context.
 */
export function KnowledgeSettingsDialog({ open, onOpenChange }: KnowledgeSettingsDialogProps) {
  const { t } = useTranslation()
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg" mobileSheet aria-describedby={undefined}>
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Settings className="h-5 w-5" />
            {t('knowledge.knowledgeSettings')}
          </DialogTitle>
        </DialogHeader>
        <KnowledgeSettings onSaved={() => onOpenChange(false)} />
      </DialogContent>
    </Dialog>
  )
}
