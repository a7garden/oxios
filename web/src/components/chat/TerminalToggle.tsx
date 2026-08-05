// TerminalToggle — header affordance for the `terminal` capability.
//
// RFC-044 Phase 3: when the active persona exposes the `terminal`
// capability, a small terminal toggle button appears in the chat header.
// Phase 3 ships only the placeholder UI ("Terminal coming soon") — the
// actual terminal surface is a follow-up milestone (RFC-044 Phase 4).

import { Terminal } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'

interface TerminalToggleProps {
  /** Optional className passthrough for the header button. */
  className?: string
}

/**
 * TerminalToggle
 *
 * Pure-UI affordance: the actual terminal is a Phase 3 follow-up. Clicking
 * the button opens a placeholder dialog that explains the state of the work
 * and lets the user close it without confusion.
 */
export function TerminalToggle({ className }: TerminalToggleProps) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)

  return (
    <>
      <Button
        type="button"
        variant="outline"
        size="sm"
        onClick={() => setOpen(true)}
        className={className ?? 'h-8 gap-1 px-2 text-xs font-normal'}
        aria-label={t('chat.terminal.title', { defaultValue: 'Terminal' })}
        title={t('chat.terminal.title', { defaultValue: 'Terminal' })}
      >
        <Terminal className="size-3.5" />
        <span>{t('chat.terminal.label', { defaultValue: 'Terminal' })}</span>
      </Button>
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              <Terminal className="size-4" />
              {t('chat.terminal.title', { defaultValue: 'Terminal' })}
            </DialogTitle>
            <DialogDescription>
              {t('chat.terminal.comingSoon', {
                defaultValue: 'Terminal coming soon',
              })}
            </DialogDescription>
          </DialogHeader>
          <div className="rounded-md border border-dashed bg-muted/30 p-4 text-xs text-muted-foreground">
            {t('chat.terminal.description', {
              defaultValue:
                'The in-chat terminal surface ships in a follow-up release. This persona already declares the capability, so the toggle is live — only the terminal pane itself is still pending.',
            })}
          </div>
        </DialogContent>
      </Dialog>
    </>
  )
}
