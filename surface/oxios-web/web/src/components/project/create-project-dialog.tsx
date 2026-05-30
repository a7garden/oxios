import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { useCreateProject } from '@/hooks/use-projects'
import type { CreateProjectInput } from '@/hooks/use-projects'
import { Button } from '@/components/ui/button'
import {
  Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { Textarea } from '@/components/ui/textarea'
import { Switch } from '@/components/ui/switch'
import { useToast } from '@/components/ui/sonner'

interface CreateProjectDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

const EMOJI_OPTIONS = ['📦', '🔧', '📝', '🎮', '🌐', '📚', '🎨', '⚡', '🎯', '🚀', '💡', '🔒', '📊', '🎪']

export function CreateProjectDialog({ open, onOpenChange }: CreateProjectDialogProps) {
  const { t } = useTranslation()
  const create = useCreateProject()
  const { toast } = useToast()

  const [name, setName] = useState('')
  const [emoji, setEmoji] = useState('📦')
  const [description, setDescription] = useState('')
  const [tags, setTags] = useState('')
  const [paths, setPaths] = useState('')
  const [memoryVisible, setMemoryVisible] = useState(true)

  const reset = () => {
    setName('')
    setEmoji('📦')
    setDescription('')
    setTags('')
    setPaths('')
    setMemoryVisible(true)
  }

  const handleSubmit = () => {
    if (!name.trim()) return

    const input: CreateProjectInput = {
      name: name.trim(),
      description: description.trim() || undefined,
      tags: tags
        .split(',')
        .map((t) => t.trim())
        .filter(Boolean),
      paths: paths
        .split(',')
        .map((p) => p.trim())
        .filter(Boolean),
      emoji,
      memory_visible: memoryVisible,
    }

    create.mutate(input, {
      onSuccess: () => {
        toast(t('projects.createSuccess', 'Project created'))
        reset()
        onOpenChange(false)
      },
      onError: (err) => {
        toast(t('projects.createError', `Failed to create project: ${err}`), { variant: 'destructive' })
      },
    })
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{t('projects.createTitle', 'New Project')}</DialogTitle>
          <DialogDescription>
            {t('projects.createDesc', 'Register a new work context.')}
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-3 py-2">
          {/* Name */}
          <div className="space-y-1">
            <label className="text-sm font-medium">{t('projects.name', 'Name')}</label>
            <Input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="oxios"
              autoFocus
            />
          </div>

          {/* Emoji */}
          <div className="space-y-1">
            <label className="text-sm font-medium">{t('projects.emoji', 'Emoji')}</label>
            <div className="flex flex-wrap gap-1">
              {EMOJI_OPTIONS.map((e) => (
                <button
                  key={e}
                  onClick={() => setEmoji(e)}
                  className={`w-8 h-8 rounded text-sm flex items-center justify-center border transition-colors ${
                    emoji === e ? 'border-primary bg-primary/10' : 'border-transparent hover:bg-muted'
                  }`}
                >
                  {e}
                </button>
              ))}
            </div>
          </div>

          {/* Description */}
          <div className="space-y-1">
            <label className="text-sm font-medium">{t('projects.description', 'Description')}</label>
            <Textarea
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder={t('projects.descriptionPlaceholder', 'Oxios Agent Operating System')}
              rows={2}
            />
          </div>

          {/* Tags */}
          <div className="space-y-1">
            <label className="text-sm font-medium">{t('projects.tags', 'Tags')}</label>
            <Input
              value={tags}
              onChange={(e) => setTags(e.target.value)}
              placeholder="rust, kernel, async"
            />
            <p className="text-[10px] text-muted-foreground">{t('projects.tagsHint', 'Comma-separated')}</p>
          </div>

          {/* Paths */}
          <div className="space-y-1">
            <label className="text-sm font-medium">{t('projects.paths', 'Paths')}</label>
            <Textarea
              value={paths}
              onChange={(e) => setPaths(e.target.value)}
              placeholder="/Volumes/MERCURY/PROJECTS/oxios"
              rows={2}
            />
            <p className="text-[10px] text-muted-foreground">
              {t('projects.pathsHint', 'One per line, or leave empty for non-code projects')}
            </p>
          </div>

          {/* Memory visible */}
          <div className="flex items-center justify-between">
            <div className="space-y-0.5">
              <label className="text-sm font-medium">{t('projects.memoryVisible', 'Memory Visible')}</label>
              <p className="text-[10px] text-muted-foreground">
                {t('projects.memoryVisibleHint', 'Allow cross-project memory access')}
              </p>
            </div>
            <Switch checked={memoryVisible} onCheckedChange={setMemoryVisible} />
          </div>
        </div>

        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            {t('common.cancel', 'Cancel')}
          </Button>
          <Button onClick={handleSubmit} disabled={!name.trim() || create.isPending}>
            {create.isPending ? '...' : t('projects.create', 'Create')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}