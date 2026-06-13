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
import { Switch } from '@/components/ui/switch'
import { Textarea } from '@/components/ui/textarea'
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

  // Sync state when project prop changes
  if (project && open) {
    if (name !== project.name) setName(project.name)
    if (icon !== (project.emoji ?? 'package')) setIcon(project.emoji ?? 'package')
    if (description !== (project.description ?? '')) setDescription(project.description ?? '')
    if (tags !== (project.tags ?? []).join(', ')) setTags((project.tags ?? []).join(', '))
    if (paths !== (project.paths ?? []).join('\n')) setPaths((project.paths ?? []).join('\n'))
    if (memoryVisible !== (project.memory_visible ?? true))
      setMemoryVisible(project.memory_visible ?? true)
  }

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
