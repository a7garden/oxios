import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { Plus, RefreshCw, Star, Trash2, Users } from 'lucide-react'
import { useState } from 'react'
import { ErrorState } from '@/components/shared/error-state'
import { EmptyState } from '@/components/shared/empty-state'
import { LoadingCards } from '@/components/shared/loading'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Textarea } from '@/components/ui/textarea'
import { api } from '@/lib/api-client'
import type { Persona } from '@/types'

export const Route = createFileRoute('/personas')({ component: PersonasPage })

function PersonasPage() {
  const queryClient = useQueryClient()
  const [showCreate, setShowCreate] = useState(false)
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
    queryFn: () => api.get<Persona[]>('/api/personas'),
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

  if (isLoading) return <LoadingCards count={4} />
  if (isError) return <ErrorState onRetry={() => refetch()} />

  const items = personas ?? []

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Personas</h1>
          <p className="text-muted-foreground">Manage agent personas</p>
        </div>
        <div className="flex gap-2">
          <Button variant="outline" size="sm" onClick={() => refetch()} disabled={isFetching}>
            <RefreshCw className={`h-4 w-4 mr-1 ${isFetching ? 'animate-spin' : ''}`} /> Refresh
          </Button>
          <Button size="sm" onClick={() => setShowCreate(true)}>
            <Plus className="h-4 w-4 mr-1" /> Create
          </Button>
        </div>
      </div>

      {showCreate && (
        <Card>
          <CardHeader>
            <CardTitle>Create Persona</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <Input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="Persona name"
            />
            <Input
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="Description"
            />
            <Textarea
              value={systemPrompt}
              onChange={(e) => setSystemPrompt(e.target.value)}
              placeholder="System prompt..."
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
                Create
              </Button>
              <Button variant="ghost" size="sm" onClick={() => setShowCreate(false)}>
                Cancel
              </Button>
            </div>
          </CardContent>
        </Card>
      )}

      {items.length === 0 && !showCreate ? (
        <EmptyState
          icon={<Users className="h-10 w-10" />}
          title="No personas"
          description="Create personas to give agents distinct personalities."
        />
      ) : (
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
          {items.map((persona) => (
            <Card key={persona.id}>
              <CardHeader className="flex flex-row items-start justify-between pb-2">
                <div>
                  <CardTitle className="text-base flex items-center gap-2">
                    <Users className="h-4 w-4" /> {persona.name}
                    {persona.active && <Star className="h-3 w-3 text-amber-500 fill-amber-500" />}
                  </CardTitle>
                  {persona.description && (
                    <p className="text-xs text-muted-foreground mt-1">{persona.description}</p>
                  )}
                </div>
                <div className="flex gap-1">
                  {!persona.active && (
                    <Button
                      variant="ghost"
                      size="icon"
                      onClick={() => activateMutation.mutate(persona.id)}
                      aria-label="Activate persona"
                    >
                      <Star className="h-4 w-4" />
                    </Button>
                  )}
                  <Button
                    variant="ghost"
                    size="icon"
                    onClick={() => deleteMutation.mutate(persona.id)}
                    aria-label="Delete persona"
                  >
                    <Trash2 className="h-4 w-4 text-destructive" />
                  </Button>
                </div>
              </CardHeader>
              {persona.system_prompt && (
                <CardContent>
                  <pre className="rounded bg-muted p-2 text-xs overflow-x-auto max-h-24">
                    {persona.system_prompt.slice(0, 200)}
                    {persona.system_prompt.length > 200 ? '...' : ''}
                  </pre>
                </CardContent>
              )}
            </Card>
          ))}
        </div>
      )}
    </div>
  )
}
