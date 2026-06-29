import {
  BarChart3,
  BookOpen,
  FileText,
  FolderOpen,
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
import { useMemo, useState } from 'react'
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
import { Textarea } from '@/components/ui/textarea'
import { useMounts } from '@/hooks/use-mounts'
import { useCreateProject, useUpdateProject } from '@/hooks/use-projects'
import { Link } from '@tanstack/react-router'

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

/** Map a project name to a sensible default icon. */
function suggestIcon(name: string): string {
  const n = name.trim().toLowerCase()
  if (!n) return 'package'
  // Common code/project keywords → matching icons.
  if (/api|server|backend|rust|kernel/.test(n)) return 'wrench'
  if (/doc|note|readme|book|knowledge/.test(n)) return 'book-open'
  if (/web|site|front|ui|app|client/.test(n)) return 'globe'
  if (/game|gaming|play/.test(n)) return 'gamepad'
  if (/design|art|style|theme|palette/.test(n)) return 'palette'
  if (/data|metric|chart|analytics|stat/.test(n)) return 'bar-chart'
  if (/launch|deploy|ship|rocket|release/.test(n)) return 'rocket'
  if (/goal|target|plan|objective/.test(n)) return 'target'
  if (/idea|think|brain|innovation/.test(n)) return 'lightbulb'
  if (/secret|secure|lock|auth|password/.test(n)) return 'lock'
  if (/camp|project|temp|scratch/.test(n)) return 'tent'
  if (/power|fast|quick|spark|energy/.test(n)) return 'zap'
  if (/file|text|doc/.test(n)) return 'file-text'
  return 'package'
}

export function CreateProjectDialog({ open, onOpenChange }: CreateProjectDialogProps) {
  const { t } = useTranslation()
  const create = useCreateProject()
  const update = useUpdateProject()
  const { data: mountsData } = useMounts()
  const availableMounts = useMemo(() => mountsData?.items ?? [], [mountsData?.items])

  const [name, setName] = useState('')
  const [icon, setIcon] = useState('package')
  const [instructions, setInstructions] = useState('')
  const [mountIds, setMountIds] = useState<string[]>([])

  const reset = () => {
    setName('')
    setIcon('package')
    setInstructions('')
    setMountIds([])
  }

  const handleNameChange = (value: string) => {
    setName(value)
    // Auto-suggest only while the user hasn't manually picked a non-default icon.
    if (icon === 'package' || icon === suggestIcon(name)) {
      setIcon(suggestIcon(value))
    }
  }

  const handleSubmit = () => {
    if (!name.trim()) return

    create.mutate(
      {
        name: name.trim(),
        emoji: icon,
        instructions: instructions.trim() || undefined,
      },
      {
        onSuccess: (created) => {
          // Attach any dropped mounts via a follow-up update — backend splits
          // Mount references from the create payload (RFC-025).
          const toAttach = mountIds
          if (toAttach.length > 0) {
            update.mutate(
              { id: created.id, mount_ids: toAttach },
              {
                onSuccess: () => {
                  toast(t('projects.createSuccess', 'Project created'))
                  reset()
                  onOpenChange(false)
                },
                onError: (err) => {
                  // Project was created; surface the attach failure but still close.
                  toast.error(
                    t(
                      'projects.attachMountsError',
                      `Project created, but attaching mounts failed: ${err}`,
                    ),
                  )
                  reset()
                  onOpenChange(false)
                },
              },
            )
            return
          }
          toast(t('projects.createSuccess', 'Project created'))
          reset()
          onOpenChange(false)
        },
        onError: (err) => {
          toast.error(t('projects.createError', `Failed to create project: ${err}`))
        },
      },
    )
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
              onChange={(e) => handleNameChange(e.target.value)}
              placeholder="oxios"
              autoFocus
            />
          </div>

          {/* Emoji (icon picker) */}
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

          {/* Instructions */}
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

          {/* RFC-025: Mount references — click-toggle chips */}
          <div className="space-y-1">
            <label className="text-sm font-medium">{t('projects.mounts', 'Mounts')}</label>
            {availableMounts.length > 0 ? (
              <div className="flex flex-wrap gap-1">
                {availableMounts.map((m) => {
                  const selected = mountIds.includes(m.id)
                  return (
                    <button
                      key={m.id}
                      type="button"
                      onClick={() =>
                        setMountIds((prev) =>
                          selected ? prev.filter((id) => id !== m.id) : [...prev, m.id],
                        )
                      }
                      className={`rounded px-2 py-1 text-xs border transition-colors ${
                        selected
                          ? 'border-primary bg-primary/10 text-primary'
                          : 'border-transparent hover:bg-muted'
                      }`}
                    >
                      <span className="inline-flex items-center gap-1">
                        <FolderOpen className="h-3 w-3" /> {m.name}
                      </span>
                    </button>
                  )
                })}
              </div>
            ) : (
              <div className="rounded-md border border-dashed p-3 text-center">
                <p className="text-xs text-muted-foreground mb-2">
                  {t('projects.noMountsYet', '마운트가 없습니다. 마운트를 먼저 만들어주세요.')}
                </p>
                <Link
                  to="/mounts"
                  onClick={() => onOpenChange(false)}
                  className="inline-flex items-center gap-1 text-xs text-primary hover:underline"
                >
                  <FolderOpen className="h-3 w-3" />
                  {t('mounts.create', 'Mount 만들기')}
                </Link>
              </div>
            )}
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