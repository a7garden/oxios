import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { createFileRoute } from '@tanstack/react-router'
import { Plus, Trash2, Zap } from 'lucide-react'
import { useState } from 'react'
import { EmptyState } from '@/components/shared/empty-state'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { RefreshButton } from '@/components/shared/refresh-button'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Input } from '@/components/ui/input'
import { Textarea } from '@/components/ui/textarea'
import { api } from '@/lib/api-client'
import type { Skill } from '@/types'

export const Route = createFileRoute('/skills')({ component: SkillsPage })

function SkillsPage() {
  const queryClient = useQueryClient()
  const [showCreate, setShowCreate] = useState(false)
  const [name, setName] = useState('')
  const [description, setDescription] = useState('')
  const [content, setContent] = useState('')

  const {
    data: skills,
    isLoading,
    isError,
    refetch,
    isFetching,
  } = useQuery({
    queryKey: ['skills'],
    queryFn: async () => {
      const res = await api.get<{ items: { name: string; description: string }[] }>('/api/skills')
      // List endpoint returns no content — must use GET /api/skills/:name for full details
      return (res.items ?? []).map((s) => ({ ...s, content: '' }))
    },
    refetchInterval: 30000,
  })

  const createMutation = useMutation({
    mutationFn: (skill: Omit<Skill, 'name'> & { name: string }) => api.post('/api/skills', skill),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['skills'] })
      setShowCreate(false)
      setName('')
      setDescription('')
      setContent('')
    },
  })

  const deleteMutation = useMutation({
    mutationFn: (skillName: string) => api.delete(`/api/skills/${skillName}`),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['skills'] }),
  })

  if (isLoading) return <LoadingCards count={4} />
  if (isError) return <ErrorState onRetry={() => refetch()} />

  const items = skills ?? []

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold">Skills</h1>
          <p className="text-muted-foreground">Manage agent skill definitions</p>
        </div>
        <div className="flex gap-2">
          <RefreshButton onClick={() => refetch()} isFetching={isFetching} />
          <Button size="sm" onClick={() => setShowCreate(true)}>
            <Plus className="h-4 w-4 mr-1" /> Create
          </Button>
        </div>
      </div>

      {showCreate && (
        <Card>
          <CardHeader>
            <CardTitle>Create Skill</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <Input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="Skill name"
            />
            <Input
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="Description (optional)"
            />
            <Textarea
              value={content}
              onChange={(e) => setContent(e.target.value)}
              placeholder="Skill content / prompt..."
              rows={6}
            />
            <div className="flex gap-2">
              <Button
                onClick={() => createMutation.mutate({ name, description, content })}
                disabled={!name.trim() || !content.trim() || createMutation.isPending}
                size="sm"
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
          icon={<Zap className="h-10 w-10" />}
          title="No skills"
          description="Create skills to give agents new capabilities."
        />
      ) : (
        <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
          {items.map((skill) => (
            <Card key={skill.name}>
              <CardHeader className="flex flex-row items-start justify-between pb-2">
                <CardTitle className="text-base flex items-center gap-2">
                  <Zap className="h-4 w-4" /> {skill.name}
                </CardTitle>
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={() => deleteMutation.mutate(skill.name)}
                  aria-label="Delete skill"
                  disabled={deleteMutation.isPending}
                >
                  <Trash2 className="h-4 w-4 text-destructive" />
                </Button>
              </CardHeader>
              <CardContent>
                {skill.description && (
                  <p className="text-sm text-muted-foreground mb-2">{skill.description}</p>
                )}
                <pre className="rounded bg-muted p-2 text-xs overflow-x-auto max-h-32">
                  {skill.content.slice(0, 200)}
                  {skill.content.length > 200 ? '...' : ''}
                </pre>
              </CardContent>
            </Card>
          ))}
        </div>
      )}
    </div>
  )
}
