import { FolderPlus } from 'lucide-react'
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
import { Label } from '@/components/ui/label'
import { useCreateMount } from '@/hooks/use-mounts'

interface CreateMountDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

/**
 * Mount 생성 다이얼로그 (RFC-025).
 *
 * 최소 입력 모델: 이름 + 경로만. 설명/태그/아이콘은 에이전트가 자동으로 채움.
 */
export function CreateMountDialog({ open, onOpenChange }: CreateMountDialogProps) {
  const { t } = useTranslation()
  const [name, setName] = useState('')
  const [path, setPath] = useState('')
  const createMount = useCreateMount()

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!name.trim() || !path.trim()) return

    try {
      await createMount.mutateAsync({ name: name.trim(), paths: [path.trim()] })
      toast.success(t('mounts.created', 'Mount가 생성되었습니다'))
      setName('')
      setPath('')
      onOpenChange(false)
    } catch (err) {
      toast.error(
        err instanceof Error ? err.message : t('mounts.createFailed', '생성 실패'),
      )
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <FolderPlus className="h-5 w-5" />
            {t('mounts.create', 'Mount 만들기')}
          </DialogTitle>
          <DialogDescription>
            {t(
              'mounts.createDescription',
              '경로에 이름을 붙입니다. 설명과 기술 스택은 에이전트가 자동으로 채웁니다.',
            )}
          </DialogDescription>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="mount-name">{t('mounts.name', '이름')}</Label>
            <Input
              id="mount-name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="oxios"
              autoFocus
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="mount-path">{t('mounts.path', '경로')}</Label>
            <Input
              id="mount-path"
              value={path}
              onChange={(e) => setPath(e.target.value)}
              placeholder="/Volumes/MERCURY/PROJECTS/oxios"
            />
          </div>
          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => onOpenChange(false)}
            >
              {t('common.cancel', '취소')}
            </Button>
            <Button
              type="submit"
              disabled={!name.trim() || !path.trim() || createMount.isPending}
            >
              {createMount.isPending
                ? t('common.creating', '생성 중...')
                : t('mounts.create', 'Mount 만들기')}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
