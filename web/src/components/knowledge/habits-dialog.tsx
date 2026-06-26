import { Activity } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { Habits } from './habits'

interface HabitsDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

/**
 * Modal wrapper for the habit / mood year-grid tracker.
 *
 * Uses a wide dialog (`sm:max-w-3xl`) so the 53-week contribution graph
 * stays legible, with bottom-sheet behavior on mobile. Replaces the former
 * full-page `/knowledge/habits` route.
 */
export function HabitsDialog({ open, onOpenChange }: HabitsDialogProps) {
  const { t } = useTranslation()
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-3xl" mobileSheet aria-describedby={undefined}>
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Activity className="h-5 w-5" />
            {t('knowledge.habitsTitle')}
          </DialogTitle>
        </DialogHeader>
        <Habits />
      </DialogContent>
    </Dialog>
  )
}
