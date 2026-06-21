import {
  BarChart3,
  BookOpen,
  FileText,
  Gamepad2,
  Globe,
  Lightbulb,
  Lock,
  Package,
  Palette,
  Rocket,
  Target,
  Tent,
  Wrench,
  Zap,
} from 'lucide-react'
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
import type { CreateProjectInput } from '@/hooks/use-projects'
import { useCreateProject } from '@/hooks/use-projects'

interface CreateProjectDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
}

/** Icon name → component mapping for project icons. */
const ICON_OPTIONS: Array<{ name: string; icon: React.ReactNode }> = [
  { name: 'package', icon: <Package className="h-4 w-4" /> },
  { name: 'wrench', icon: <Wrench className="h-4 w-4" /> },
  { name: 'file-text', icon: <FileText className="h-4 w-4" /> },
  { name: 'gamepad', icon: <Gamepad2 className="h-4 w-4" /> },
  { name: 'globe', icon: <Globe className="h-4 w-4" /> },
  { name: 'book-open', icon: <BookOpen className="h-4 w-4" /> },
  { name: 'palette', icon: <Palette className="h-4 w-4" /> },
  { name: 'zap', icon: <Zap className="h-4 w-4" /> },
  { name: 'target', icon: <Target className="h-4 w-4" /> },
  { name: 'rocket', icon: <Rocket className="h-4 w-4" /> },
  { name: 'lightbulb', icon: <Lightbulb className="h-4 w-4" /> },
  { name: 'lock', icon: <Lock className="h-4 w-4" /> },
  { name: 'bar-chart', icon: <BarChart3 className="h-4 w-4" /> },
  { name: 'tent', icon: <Tent className="h-4 w-4" /> },
]

export { ICON_OPTIONS }

export function CreateProjectDialog({ open, onOpenChange }: CreateProjectDialogProps) {
  const { t } = useTranslation()
  const create = useCreateProject()

  const [name, setName] = useState('')
  const [icon, setIcon] = useState('package')
  const [description, setDescription] = useState('')
  const [tags, setTags] = useState('')
  const [paths, setPaths] = useState('')
  const [memoryVisible, setMemoryVisible] = useState(true)

  const reset = () => {
    setName('')
    setIcon('package')
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
      emoji: icon,
      memory_visible: memoryVisible,
    }

    create.mutate(input, {
      onSuccess: () => {
        toast(t('projects.createSuccess', 'Project created'))
        reset()
        onOpenChange(false)
      },
      onError: (err) => {
        toast.error(t('projects.createError', `Failed to create project: ${err}`))
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

        <div className="space-y-4 py-2">
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

          {/* Icon */}
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

          {/* Description */}
          <div className="space-y-1">
            <label className="text-sm font-medium">
              {t('projects.description', 'Description')}
            </label>
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
            <p className="text-2xs text-muted-foreground">
              {t('projects.tagsHint', 'Comma-separated')}
            </p>
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
            <p className="text-2xs text-muted-foreground">
              {t('projects.pathsHint', 'One per line, or leave empty for non-code projects')}
            </p>
          </div>

          {/* Memory visible */}
          <div className="flex items-center justify-between">
            <div className="space-y-0.5">
              <label className="text-sm font-medium">
                {t('projects.memoryVisible', 'Memory Visible')}
              </label>
              <p className="text-2xs text-muted-foreground">
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
