import { Pencil } from 'lucide-react'
import { useEffect, useState } from 'react'
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
import { Label } from '@/components/ui/label'
import { useUpdateMount } from '@/hooks/use-mounts'
import type { Mount } from '@/types'

interface EditMountDialogProps {
  /** The mount being edited, or null when closed. */
  mount: Mount | null
  onOpenChange: (mount: Mount | null) => void
}

/**
 * Mount 편집 다이얼로그 (RFC-025).
 *
 * 마운트의 이름(별칭)과 경로를 모두 편집할 수 있습니다. 경로가 바뀌면
 * 캐시된 자동 설명/기술 스택이 무효화되어 '갱신 필요'로 표시됩니다.
 */
export function EditMountDialog({ mount, onOpenChange }: EditMountDialogProps) {
  const { t } = useTranslation()
  const [name, setName] = useState('')
  const [path, setPath] = useState('')
  const updateMount = useUpdateMount()

  // 대상 마운트가 바뀌거나 다이얼로그가 열릴 때 로컬 필드 동기화.
  useEffect(() => {
    if (mount) {
      setName(mount.name)
      setPath(mount.paths[0] ?? '')
    }
  }, [mount])

  const close = () => onOpenChange(null)

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!mount) return
    const trimmedName = name.trim()
    const trimmedPath = path.trim()
    const unchanged = trimmedName === mount.name && trimmedPath === (mount.paths[0] ?? '')
    if (unchanged) {
      close()
      return
    }

    try {
      await updateMount.mutateAsync({ id: mount.id, name: trimmedName, paths: [trimmedPath] })
      toast.success(t('mounts.saved'))
      close()
    } catch (err) {
      toast.error(err instanceof Error ? err.message : t('mounts.saveFailed'))
    }
  }

  return (
    <Dialog open={mount !== null} onOpenChange={(o) => !o && close()}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Pencil className="h-5 w-5" />
            {t('mounts.edit')}
          </DialogTitle>
          <DialogDescription>{t('mounts.editDescription')}</DialogDescription>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="mount-name-edit">{t('mounts.name')}</Label>
            <Input
              id="mount-name-edit"
              value={name}
              onChange={(e) => setName(e.target.value)}
              autoFocus
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="mount-path-edit">{t('mounts.path')}</Label>
            <Input
              id="mount-path-edit"
              value={path}
              onChange={(e) => setPath(e.target.value)}
              className="font-mono text-sm"
              placeholder="/path/to/project"
            />
            <p className="text-xs text-muted-foreground">{t('mounts.pathEditHint')}</p>
          </div>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={close}>
              {t('common.cancel')}
            </Button>
            <Button type="submit" disabled={!name.trim() || !path.trim() || updateMount.isPending}>
              {updateMount.isPending ? t('common.saving') : t('common.save')}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
