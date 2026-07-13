import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'

interface NewFolderDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** Pre-filled parent directory. Empty string or undefined = root. */
  parentPath?: string
  onConfirm: (folderName: string) => void | Promise<void>
}

/**
 * Phase 4 follow-up: replaces the legacy `window.prompt('Enter folder name:')`.
 * Folders are persisted as a 0-byte `.keep` file inside a directory (the
 * directory itself isn't tracked on its own).
 */
export function NewFolderDialog({
  open,
  onOpenChange,
  parentPath,
  onConfirm,
}: NewFolderDialogProps) {
  const { t } = useTranslation()
  const [name, setName] = useState('New Folder')
  const [busy, setBusy] = useState(false)

  const handleCreate = async () => {
    const trimmed = name.trim()
    if (!trimmed) return
    setBusy(true)
    try {
      await onConfirm(trimmed)
      onOpenChange(false)
    } catch (err) {
      toast.error(err instanceof Error ? err.message : t('knowledge.createFolderFailed'))
    } finally {
      setBusy(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{t('knowledge.newFolderDialogTitle')}</DialogTitle>
          <DialogDescription>
            {parentPath
              ? t('knowledge.newFolderInPath', { path: parentPath })
              : t('knowledge.newFolderAtRoot')}
          </DialogDescription>
        </DialogHeader>
        <Input
          autoFocus
          value={name}
          onChange={(e) => setName(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault()
              void handleCreate()
            }
          }}
          placeholder={t('knowledge.newFolderPlaceholder')}
          disabled={busy}
        />
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)} disabled={busy}>
            {t('common.cancel')}
          </Button>
          <Button onClick={() => void handleCreate()} disabled={busy}>
            {t('knowledge.create')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
