import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { Pencil, Plus, Star, Trash2, Users } from 'lucide-react'
import { useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { EditPersonaDialog, type PersonaItem, type PersonaPatch } from '@/components/persona/edit-persona-dialog'
import { EmptyState } from '@/components/shared/empty-state'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { RefreshButton } from '@/components/shared/refresh-button'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Textarea } from '@/components/ui/textarea'
import { api } from '@/lib/api-client'

export const Route = createFileRoute('/personas')({ component: PersonasPage })

function PersonasPage() {
  const { t } = useTranslation()
  const queryClient = useQueryClient()
  const [showCreate, setShowCreate] = useState(false)
  const [editing, setEditing] = useState<PersonaItem | null>(null)
  const [name, setName] = useState('')
  const [description, setDescription] = useState('')
  const [systemPrompt, setSystemPrompt] = useState('')

  const {
    data: personas,
    isLoading,
    isError,
    refetch,
    isFetching,
  } = useQuery({
    queryKey: ['personas'],
    queryFn: async () => {
      const res =
        await api.get<
          {
            id: string
            name: string
            role: string
            description: string
            enabled: boolean
            personality_traits: string[]
          }[]
        >('/api/personas')
      // Backend returns raw array
      return Array.isArray(res) ? res : []
    },
    refetchInterval: 30000,
  })

  const createMutation = useMutation({
    mutationFn: (p: { name: string; description: string; system_prompt: string }) =>
      api.post('/api/personas', p),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['personas'] })
      setShowCreate(false)
      setName('')
      setDescription('')
      setSystemPrompt('')
    },
  })

  const deleteMutation = useMutation({
    mutationFn: (id: string) => api.delete(`/api/personas/${id}`),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['personas'] }),
  })

  const activateMutation = useMutation({
    mutationFn: (id: string) => api.post(`/api/personas/${id}/activate`),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['personas'] }),
  })

  const updateMutation = useMutation({
    mutationFn: ({ id, patch }: { id: string; patch: PersonaPatch }) =>
      api.put(`/api/personas/${id}`, patch),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['personas'] })
      setEditing(null)
      toast.success(t('personas.saved', '페르소나가 저장되었습니다'))
    },
    onError: (err) => {
      toast.error(err instanceof Error ? err.message : t('personas.saveFailed', '저장 실패'))
    },
  })

  if (isLoading) return <LoadingCards count={4} />
  if (isError) return <ErrorState onRetry={() => refetch()} />

  const items = Array.isArray(personas) ? personas : []

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">{t('personas.title')}</h1>
          <p className="text-muted-foreground">{t('personas.subtitle')}</p>
        </div>
        <div className="flex gap-2">
          <RefreshButton onClick={() => refetch()} isFetching={isFetching} />
          <Button size="sm" onClick={() => setShowCreate(true)}>
            <Plus className="h-4 w-4 mr-1" /> {t('common.create')}
          </Button>
        </div>
      </div>

      {showCreate && (
        <Card>
          <CardHeader>
            <CardTitle>{t('personas.createPersona')}</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <Input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder={t('personas.personaNamePlaceholder')}
            />
            <Input
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder={t('common.description')}
            />
            <Textarea
              value={systemPrompt}
              onChange={(e) => setSystemPrompt(e.target.value)}
              placeholder={t('personas.systemPromptPlaceholder')}
              rows={4}
            />
            <div className="flex gap-2">
              <Button
                size="sm"
                onClick={() =>
                  createMutation.mutate({ name, description, system_prompt: systemPrompt })
                }
                disabled={!name.trim() || createMutation.isPending}
              >
                {t('common.create')}
              </Button>
              <Button variant="ghost" size="sm" onClick={() => setShowCreate(false)}>
                {t('common.cancel')}
              </Button>
            </div>
          </CardContent>
        </Card>
      )}

      {items.length === 0 && !showCreate ? (
        <EmptyState
          icon={<Users className="h-10 w-10" />}
          title={t('personas.noPersonas')}
          description={t('personas.descriptionHint')}
        />
      ) : (
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
          {items.map((persona) => (
            <Card key={persona.id}>
              <CardHeader className="flex flex-row items-start justify-between pb-2">
                <div>
                  <CardTitle className="text-base flex items-center gap-2">
                    <Users className="h-4 w-4" /> {persona.name}
                    {persona.enabled && <Star className="h-3 w-3 text-warning fill-warning" />}
                  </CardTitle>
                  {persona.description && (
                    <p className="text-xs text-muted-foreground mt-1">{persona.description}</p>
                  )}
                </div>
                <div className="flex gap-1">
                  <Button
                    variant="ghost"
                    size="icon"
                    onClick={() => setEditing(persona)}
                    aria-label={t('common.edit', '편집')}
                  >
                    <Pencil className="h-4 w-4" />
                  </Button>
                  {!persona.enabled && (
                    <Button
                      variant="ghost"
                      size="icon"
                      onClick={() => activateMutation.mutate(persona.id)}
                      aria-label={t('personas.activatePersona')}
                    >
                      <Star className="h-4 w-4" />
                    </Button>
                  )}
                  <Button
                    variant="ghost"
                    size="icon"
                    onClick={() => deleteMutation.mutate(persona.id)}
                    aria-label={t('personas.deletePersona')}
                  >
                    <Trash2 className="h-4 w-4 text-destructive" />
                  </Button>
                </div>
              </CardHeader>
              {persona.role && (
                <CardContent>
                  <p className="text-xs text-muted-foreground">
                    {t('personas.role')}: {persona.role}
                  </p>
                </CardContent>
              )}
            </Card>
          ))}
        </div>
      )}

      <EditPersonaDialog
        persona={editing}
        isPending={updateMutation.isPending}
        onOpenChange={(open) => !open && setEditing(null)}
        onSave={(patch) => {
          if (!editing) return
          updateMutation.mutate({ id: editing.id, patch })
        }}
      />
    </div>
  )
}
