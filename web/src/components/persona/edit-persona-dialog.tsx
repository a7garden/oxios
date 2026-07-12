import { useQuery } from '@tanstack/react-query'
import { Pencil } from 'lucide-react'
import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
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
import { Textarea } from '@/components/ui/textarea'
import { api } from '@/lib/api-client'

export interface PersonaItem {
  id: string
  name: string
  role: string
  description: string
  enabled: boolean
  personality_traits?: string[]
}

export interface PersonaPatch {
  name: string
  description: string
  system_prompt: string
}

interface EditPersonaDialogProps {
  persona: PersonaItem | null
  isPending: boolean
  onOpenChange: (open: boolean) => void
  onSave: (patch: PersonaPatch) => void
}

interface PersonaDetail {
  id: string
  name: string
  role: string
  description: string
  system_prompt: string
  enabled: boolean
  personality_traits: string[]
}

/**
 * Persona 편집 다이얼로그. 백엔드 PUT /api/personas/:id 로 부분 업데이트.
 *
 * 리스트 응답에는 system_prompt 가 없으므로 (wipe 방지) 열릴 때
 * GET /api/personas/:id 로 전체를 가져와 system_prompt 까지 prefill 합니다.
 * 사용자가 system_prompt 를 수정하지 않은 경우에도 보내지만, 백엔드는
 * Some("") 와 Some(prev) 를 구분하지 못하므로 — 따라서 사용자가 textarea
 * 를 건드리지 않으면 원본을 그대로 보냅니다.
 */
export function EditPersonaDialog({
  persona,
  isPending,
  onOpenChange,
  onSave,
}: EditPersonaDialogProps) {
  const { t } = useTranslation()
  const [name, setName] = useState('')
  const [description, setDescription] = useState('')
  const [systemPrompt, setSystemPrompt] = useState('')

  const detail = useQuery({
    queryKey: ['persona', persona?.id],
    queryFn: () => api.get<PersonaDetail>(`/api/personas/${persona!.id}`),
    enabled: persona !== null,
  })

  // 대상 페르소나가 바뀌거나 상세가 로딩되면 로컬 필드 동기화.
  useEffect(() => {
    if (!persona) return
    if (detail.data) {
      setName(detail.data.name)
      setDescription(detail.data.description)
      setSystemPrompt(detail.data.system_prompt)
    } else {
      setName(persona.name)
      setDescription(persona.description)
    }
  }, [persona, detail.data])

  const close = () => onOpenChange(false)

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (!persona) return
    const n = name.trim()
    if (!n) return
    onSave({ name: n, description: description.trim(), system_prompt: systemPrompt })
  }

  return (
    <Dialog open={persona !== null} onOpenChange={(o) => !o && close()}>
      <DialogContent className="max-w-md">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Pencil className="h-5 w-5" />
            {t('personas.edit')}
          </DialogTitle>
          <DialogDescription>{t('personas.editDescription')}</DialogDescription>
        </DialogHeader>
        {detail.isLoading ? (
          <div className="text-sm text-muted-foreground p-4 text-center">{t('common.loading')}</div>
        ) : detail.isError ? (
          <div className="space-y-3 p-2">
            <p className="text-sm text-destructive">{t('personas.loadFailed')}</p>
            <Button
              type="button"
              variant="outline"
              onClick={() => detail.refetch()}
              disabled={detail.isFetching}
            >
              {t('common.retry')}
            </Button>
          </div>
        ) : (
          <form onSubmit={handleSubmit} className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="persona-edit-name">{t('personas.personaName')}</Label>
              <Input
                id="persona-edit-name"
                value={name}
                onChange={(e) => setName(e.target.value)}
                autoFocus
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="persona-edit-desc">{t('common.description')}</Label>
              <Input
                id="persona-edit-desc"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="persona-edit-prompt">{t('personas.systemPrompt')}</Label>
              <Textarea
                id="persona-edit-prompt"
                value={systemPrompt}
                onChange={(e) => setSystemPrompt(e.target.value)}
                rows={6}
              />
            </div>
            <DialogFooter>
              <Button type="button" variant="outline" onClick={close}>
                {t('common.cancel')}
              </Button>
              <Button type="submit" disabled={!name.trim() || isPending}>
                {isPending ? t('common.saving') : t('common.save')}
              </Button>
            </DialogFooter>
          </form>
        )}
      </DialogContent>
    </Dialog>
  )
}
