import { useMutation, useQueryClient } from '@tanstack/react-query'
import { Plus } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { api } from '@/lib/api-client'
import type { Skill } from '@/types'

interface SkillEditorDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** When set, the dialog edits this skill; otherwise it creates a new one. */
  skill?: Skill | null
  /** Initial content for edit mode (fetched SKILL.md raw). */
  initialContent?: string
}

const NAME_RE = /^[a-z0-9][a-z0-9-]{0,63}$/

/**
 * Create / edit skill modal (design F1 + F3).
 *
 * - Create: POST /api/skills (name/description/body) — frontmatter is
 *   synthesized, which is the intended behavior for brand-new skills.
 * - Edit:   PUT /api/skills/{name}/content — writes the raw SKILL.md verbatim
 *   so rich frontmatter is preserved (create_skill would strip it).
 */
export function SkillEditorDialog({
  open,
  onOpenChange,
  skill,
  initialContent = '',
}: SkillEditorDialogProps) {
  const { t } = useTranslation()
  const qc = useQueryClient()
  const isEdit = !!skill

  const [name, setName] = useState(skill?.name ?? '')
  const [description, setDescription] = useState(skill?.description ?? '')
  const [body, setBody] = useState(initialContent)

  // Re-seed when the dialog opens for a different skill.
  const seedKey = `${open}:${skill?.name ?? 'new'}`
  const [lastSeed, setLastSeed] = useState('')
  if (open && seedKey !== lastSeed) {
    setName(skill?.name ?? '')
    setDescription(skill?.description ?? '')
    setBody(initialContent)
    setLastSeed(seedKey)
  }

  const nameValid = NAME_RE.test(name)
  const descValid = description.trim().length > 0 && description.length <= 1024
  const canSave = nameValid && descValid && (isEdit || body.trim().length > 0)

  const createMut = useMutation({
    mutationFn: (vars: { name: string; description: string; content: string }) =>
      api.post<{ name: string }>('/api/skills', vars),
    onSuccess: () => {
      toast.success(t('skills.saveSuccess'))
      qc.invalidateQueries({ queryKey: ['skills'] })
      onOpenChange(false)
    },
    onError: (e: unknown) => toast.error(e instanceof Error ? e.message : t('skills.saveFailed')),
  })

  const updateMut = useMutation({
    mutationFn: (vars: { content: string }) =>
      api.put<Skill>(`/api/skills/${encodeURIComponent(skill!.name)}/content`, {
        content: vars.content,
      }),
    onSuccess: () => {
      toast.success(t('skills.saveSuccess'))
      qc.invalidateQueries({ queryKey: ['skills'] })
      qc.invalidateQueries({ queryKey: ['skill', skill!.name, 'content'] })
      onOpenChange(false)
    },
    onError: (e: unknown) => toast.error(e instanceof Error ? e.message : t('skills.saveFailed')),
  })

  const saving = createMut.isPending || updateMut.isPending

  const handleSave = () => {
    if (!canSave) return
    if (isEdit) {
      updateMut.mutate({ content: body })
    } else {
      createMut.mutate({ name, description, content: body })
    }
  }

  // Frontmatter preview for create mode.
  const fmPreview = isEdit
    ? null
    : `---\nname: ${name || '<name>'}\ndescription: ${description || '<description>'}\n---`

  return (
    <>
      {open && (
        <div
          className="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-6"
          onClick={() => onOpenChange(false)}
        >
          <div
            className="bg-card border rounded-xl w-full max-w-3xl max-h-[92vh] flex flex-col overflow-hidden shadow-lg"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="flex items-center gap-3 px-6 py-4 border-b">
              <span className="font-mono text-[10px] uppercase tracking-wider text-primary bg-primary/10 px-2 py-0.5 rounded-full">
                {isEdit ? 'edit' : 'create'}
              </span>
              <h2 className="font-semibold text-lg">
                {isEdit ? t('skills.editTitle', { name: skill!.name }) : t('skills.createTitle')}
              </h2>
            </div>

            <div className="p-6 overflow-y-auto flex-1 space-y-4">
              {!isEdit && (
                <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                  <div>
                    <label className="block text-sm font-semibold mb-1.5">
                      {t('skills.nameLabel')} <span className="text-destructive">*</span>
                    </label>
                    <input
                      className="font-mono text-sm w-full bg-background border rounded-md px-3 py-2 outline-none focus:border-ring focus:ring-2 focus:ring-ring/20"
                      placeholder="code-review"
                      value={name}
                      onChange={(e) => setName(e.target.value)}
                    />
                    <p className="text-xs text-muted-foreground mt-1 font-mono">
                      {t('skills.nameHint')}
                    </p>
                  </div>
                  <div>
                    <label className="block text-sm font-semibold mb-1.5">
                      {t('skills.descriptionLabel')} <span className="text-destructive">*</span>
                    </label>
                    <input
                      className="w-full bg-background border rounded-md px-3 py-2 outline-none focus:border-ring focus:ring-2 focus:ring-ring/20"
                      placeholder={t('skills.descriptionHint')}
                      value={description}
                      onChange={(e) => setDescription(e.target.value)}
                    />
                    <p className="text-xs text-muted-foreground mt-1 font-mono">
                      {t('skills.descriptionHint')}
                    </p>
                  </div>
                </div>
              )}

              {isEdit && (
                <div className="rounded-md bg-info/10 border border-info/20 px-3 py-2 text-xs text-info">
                  {t('skills.editMarketplaceWarning')}
                </div>
              )}

              <div>
                <label className="block text-sm font-semibold mb-1.5">
                  {t('skills.bodyLabel')}
                </label>
                <textarea
                  className="font-mono text-xs w-full bg-background border rounded-md px-3 py-3 outline-none focus:border-ring focus:ring-2 focus:ring-ring/20 min-h-[280px] resize-y"
                  spellCheck={false}
                  value={body}
                  onChange={(e) => setBody(e.target.value)}
                />
              </div>

              {fmPreview && (
                <div className="bg-background border border-dashed rounded-md px-4 py-3 font-mono text-xs text-muted-foreground whitespace-pre-wrap leading-relaxed">
                  {fmPreview}
                </div>
              )}
            </div>

            <div className="flex items-center gap-2 px-6 py-4 border-t bg-background">
              <div className="flex-1" />
              <Button variant="ghost" onClick={() => onOpenChange(false)} disabled={saving}>
                {t('common.cancel')}
              </Button>
              <Button onClick={handleSave} disabled={!canSave || saving}>
                <Plus className="h-4 w-4" />
                {isEdit ? t('common.save') : t('skills.saveActivate')}
              </Button>
            </div>
          </div>
        </div>
      )}
    </>
  )
}
