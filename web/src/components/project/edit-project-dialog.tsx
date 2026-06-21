import { FolderOpen } from 'lucide-react'
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
import { Switch } from '@/components/ui/switch'
import { Textarea } from '@/components/ui/textarea'
import { useMounts } from '@/hooks/use-mounts'
import { useUpdateProject } from '@/hooks/use-projects'
import type { Project } from '@/types'

interface EditProjectDialogProps {
  project: Project | null
  open: boolean
  onOpenChange: (open: boolean) => void
  onSuccess?: () => void
}

import { ICON_OPTIONS } from './create-project-dialog'

export function EditProjectDialog({
  project,
  open,
  onOpenChange,
  onSuccess,
}: EditProjectDialogProps) {
  const { t } = useTranslation()
  const update = useUpdateProject()

  const [name, setName] = useState('')
  const [icon, setIcon] = useState('package')
  const [description, setDescription] = useState('')
  const [tags, setTags] = useState('')
  const [paths, setPaths] = useState('')
  const [memoryVisible, setMemoryVisible] = useState(true)
  // RFC-025: mount_ids + instructions
  const [mountIds, setMountIds] = useState<string[]>([])
  const [instructions, setInstructions] = useState('')
  const { data: mountsData } = useMounts()
  const availableMounts = mountsData?.items ?? []

  // Sync state when the project prop or open state changes.
  useEffect(() => {
    if (project && open) {
      setName(project.name)
      setIcon(project.emoji ?? 'package')
      setDescription(project.description ?? '')
      setTags((project.tags ?? []).join(', '))
      setPaths((project.paths ?? []).join('\n'))
      setMemoryVisible(project.memory_visible ?? true)
      setMountIds(project.mount_ids ?? [])
      setInstructions(project.instructions ?? '')
    }
  }, [project?.id, open])

  const handleSubmit = () => {
    if (!project || !name.trim()) return

    update.mutate(
      {
        id: project.id,
        name: name.trim(),
        description: description.trim() || undefined,
        tags: tags
          .split(',')
          .map((t) => t.trim())
          .filter(Boolean),
        paths: paths
          .split('\n')
          .map((p) => p.trim())
          .filter(Boolean),
        emoji: icon,
        memory_visible: memoryVisible,
        mount_ids: mountIds,
        instructions: instructions.trim() || undefined,
      },
      {
        onSuccess: () => {
          toast(t('projects.updateSuccess', 'Project updated'))
          onOpenChange(false)
          onSuccess?.()
        },
        onError: (err) => {
          toast.error(t('projects.updateError', `Failed to update: ${err}`))
        },
      },
    )
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{t('projects.editTitle', 'Edit Project')}</DialogTitle>
          <DialogDescription>
            {project?.name
              ? t('projects.editDesc', 'Update "{{name}}"', { name: project.name })
              : ''}
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4 py-2">
          <div className="space-y-1">
            <label className="text-sm font-medium">{t('projects.name', 'Name')}</label>
            <Input value={name} onChange={(e) => setName(e.target.value)} placeholder="oxios" />
          </div>

          <div className="space-y-1">
            <label className="text-sm font-medium">{t('projects.icon', 'Icon')}</label>
            <div className="flex flex-wrap gap-1">
              {ICON_OPTIONS.map((opt) => (
                <button
                  key={opt.name}
                  type="button"
                  onClick={() => setIcon(opt.name)}
                  className={`w-8 h-8 rounded flex items-center justify-center border transition-colors ${
                    icon === opt.name
                      ? 'border-primary bg-primary/10'
                      : 'border-transparent hover:bg-muted'
                  }`}
                >
                  {opt.icon}
                </button>
              ))}
            </div>
          </div>

          <div className="space-y-1">
            <label className="text-sm font-medium">
              {t('projects.description', 'Description')}
            </label>
            <Textarea
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              rows={2}
            />
          </div>

          <div className="space-y-1">
            <label className="text-sm font-medium">{t('projects.tags', 'Tags')}</label>
            <Input value={tags} onChange={(e) => setTags(e.target.value)} />
            <p className="text-2xs text-muted-foreground">
              {t('projects.tagsHint', 'Comma-separated')}
            </p>
          </div>

          <div className="space-y-1">
            <label className="text-sm font-medium">{t('projects.paths', 'Paths')}</label>
            <Textarea value={paths} onChange={(e) => setPaths(e.target.value)} rows={2} />
            <p className="text-2xs text-muted-foreground">
              {t('projects.pathsHint', 'One per line')}
            </p>
          </div>

          <div className="flex items-center justify-between">
            <label className="text-sm font-medium">
              {t('projects.memoryVisible', 'Memory Visible')}
            </label>
            <Switch checked={memoryVisible} onCheckedChange={setMemoryVisible} />
          </div>

          {/* RFC-025: Mount references */}
          {availableMounts.length > 0 && (
            <div className="space-y-1">
              <label className="text-sm font-medium">{t('projects.mounts', 'Mounts')}</label>
              <div className="flex flex-wrap gap-1">
                {availableMounts.map((m) => (
                  <button
                    key={m.id}
                    type="button"
                    onClick={() => {
                      setMountIds((prev) =>
                        prev.includes(m.id) ? prev.filter((id) => id !== m.id) : [...prev, m.id],
                      )
                    }}
                    className={`rounded px-2 py-1 text-xs border transition-colors ${
                      mountIds.includes(m.id)
                        ? 'border-primary bg-primary/10 text-primary'
                        : 'border-transparent hover:bg-muted'
                    }`}
                  >
                    <span className="inline-flex items-center gap-1">
                      <FolderOpen className="h-3 w-3" /> {m.name}
                    </span>
                  </button>
                ))}
              </div>
            </div>
          )}

          {/* RFC-025: Custom instructions */}
          <div className="space-y-1">
            <label className="text-sm font-medium">
              {t('projects.instructions', 'Instructions')}
            </label>
            <Textarea
              value={instructions}
              onChange={(e) => setInstructions(e.target.value)}
              rows={3}
              placeholder={t(
                'projects.instructionsPlaceholder',
                '이 Project에서 항상 지켜야 할 규칙. 시스템 프롬프트에 주입됩니다.',
              )}
            />
          </div>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            {t('common.cancel', 'Cancel')}
          </Button>
          <Button onClick={handleSubmit} disabled={!name.trim() || update.isPending}>
            {update.isPending ? '...' : t('projects.save', 'Save')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
