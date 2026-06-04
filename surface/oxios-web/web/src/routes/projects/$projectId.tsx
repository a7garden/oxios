import { useState } from 'react'
import { createFileRoute, useNavigate } from '@tanstack/react-router'
import { useTranslation } from 'react-i18next'
import { ArrowLeft, Edit, Trash2 } from 'lucide-react'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Badge } from '@/components/ui/badge'
import { useProject, useProjectMemories } from '@/hooks/use-projects'
import { EditProjectDialog } from '@/components/project/edit-project-dialog'
import { DeleteProjectDialog } from '@/components/project/delete-project-dialog'
import type { Project } from '@/types'

export const Route = createFileRoute('/projects/$projectId')({
  component: ProjectDetailPage,
})

const SOURCE_COLORS: Record<string, string> = {
  manual: 'bg-emerald-100 text-emerald-700',
  auto_detected: 'bg-amber-100 text-amber-700',
}

function formatDate(iso: string) {
  return new Date(iso).toLocaleString()
}

function formatRelativeTime(iso: string): string {
  const diff = Date.now() - new Date(iso).getTime()
  const mins = Math.floor(diff / 60000)
  const hours = Math.floor(diff / 3600000)
  const days = Math.floor(diff / 86400000)
  if (mins < 1) return 'just now'
  if (mins < 60) return `${mins}m ago`
  if (hours < 24) return `${hours}h ago`
  return `${days}d ago`
}

function ProjectPathsCard({ project }: { project: Project }) {
  const { t } = useTranslation()

  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base">{t('projects.paths', 'Paths')}</CardTitle>
      </CardHeader>
      <CardContent>
        {project.paths && project.paths.length > 0 ? (
          <div className="space-y-2">
            {project.paths.map((path, i) => (
              <div key={i} className="flex items-center gap-2 text-sm">
                <span>📁</span>
                <code className="text-xs bg-muted px-2 py-1 rounded font-mono truncate">{path}</code>
              </div>
            ))}
          </div>
        ) : (
          <p className="text-sm text-muted-foreground">
            {t('projects.noPaths', 'No paths — this is a non-code project')}
          </p>
        )}
      </CardContent>
    </Card>
  )
}

function ProjectDetailsCard({ project }: { project: Project }) {
  const { t } = useTranslation()
  const sourceColor = SOURCE_COLORS[project.source ?? 'manual'] ?? SOURCE_COLORS.manual

  const details = [
    { label: t('projects.source', 'Source'), value: project.source ?? 'manual' },
    { label: t('projects.memoryVisible', 'Memory Visible'), value: project.memory_visible ? '✅' : '❌' },
    { label: t('projects.createdAt', 'Created'), value: formatDate(project.created_at) },
    { label: t('projects.updatedAt', 'Updated'), value: formatDate(project.updated_at ?? project.created_at) },
    {
      label: t('projects.lastActive', 'Last Active'),
      value: formatRelativeTime(project.last_active_at ?? project.updated_at ?? project.created_at),
    },
  ]

  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base">{t('projects.details', 'Details')}</CardTitle>
      </CardHeader>
      <CardContent>
        <div className="space-y-3">
          {/* Description */}
          {project.description && (
            <div>
              <p className="text-xs text-muted-foreground mb-1">{t('projects.description', 'Description')}</p>
              <p className="text-sm">{project.description}</p>
            </div>
          )}

          {/* Tags */}
          {project.tags && project.tags.length > 0 && (
            <div>
              <p className="text-xs text-muted-foreground mb-1">{t('projects.tags', 'Tags')}</p>
              <div className="flex flex-wrap gap-1">
                {project.tags.map((tag) => (
                  <Badge key={tag} variant="secondary" className="text-xs">
                    {tag}
                  </Badge>
                ))}
              </div>
            </div>
          )}

          {/* Other details */}
          <div className="grid gap-2 text-sm">
            {details.map((d) => (
              <div key={d.label} className="flex items-center justify-between">
                <span className="text-muted-foreground text-xs">{d.label}</span>
                <span className={d.label === 'Source' ? `text-[10px] px-1.5 py-0.5 rounded ${sourceColor}` : 'text-xs'}>
                  {d.value}
                </span>
              </div>
            ))}
          </div>
        </div>
      </CardContent>
    </Card>
  )
}

function ProjectMemoriesCard({ project }: { project: Project }) {
  const { t } = useTranslation()
  const { data, isLoading } = useProjectMemories(project.id)

  const memories = data?.items ?? []

  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base flex items-center gap-2">
          {t('projects.memories', 'Memories')}
          {memories.length > 0 && (
            <Badge variant="secondary" className="text-xs">{memories.length}</Badge>
          )}
        </CardTitle>
      </CardHeader>
      <CardContent>
        {isLoading ? (
          <div className="space-y-2">
            {[1, 2, 3].map((i) => (
              <div key={i} className="h-8 bg-muted rounded animate-pulse" />
            ))}
          </div>
        ) : memories.length === 0 ? (
          <p className="text-sm text-muted-foreground">
            {t('projects.noMemories', 'No memories linked to this project')}
          </p>
        ) : (
          <div className="space-y-2">
            {memories.map((mem: any) => (
              <div key={mem.id} className="flex items-center gap-2 p-2 rounded bg-muted/50">
                <Badge variant="outline" className="text-[10px] shrink-0">
                  {mem.memory_type ?? mem.tier ?? 'memory'}
                </Badge>
                <p className="text-xs truncate flex-1">{mem.content?.slice(0, 80)}...</p>
              </div>
            ))}
          </div>
        )}
      </CardContent>
    </Card>
  )
}

function ProjectActivityCard() {
  const { t } = useTranslation()

  return (
    <Card>
      <CardHeader>
        <CardTitle className="text-base">{t('projects.activity', 'Activity')}</CardTitle>
      </CardHeader>
      <CardContent>
        <p className="text-sm text-muted-foreground">
          {t('projects.activityDesc', 'Session history and memory changes — coming in Phase 3')}
        </p>
      </CardContent>
    </Card>
  )
}

function ProjectDetailPage() {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const { projectId } = Route.useParams()

  const { data: project, isLoading, isError, refetch } = useProject(projectId)

  const [showEdit, setShowEdit] = useState(false)
  const [showDelete, setShowDelete] = useState(false)

  if (isLoading) return <LoadingCards count={4} />
  if (isError) return <ErrorState onRetry={() => refetch()} />
  if (!project) return <p className="text-muted-foreground">{t('projects.notFound', 'Project not found')}</p>

  return (
    <div className="space-y-4">
      {/* Back + Header */}
      <div className="flex items-center gap-4">
        <Button
          variant="ghost"
          size="icon"
          onClick={() => navigate({ to: '/projects' })}
          aria-label={t('common.back', 'Back')}
        >
          <ArrowLeft className="h-4 w-4" />
        </Button>
        <div className="flex items-center gap-2 flex-1 min-w-0">
          <span className="text-2xl">{project.emoji ?? '📦'}</span>
          <h1 className="text-2xl font-bold truncate">{project.name}</h1>
        </div>
        <div className="flex items-center gap-1 shrink-0">
          <Button variant="outline" size="sm" onClick={() => setShowEdit(true)}>
            <Edit className="h-3 w-3 mr-1" />
            {t('common.edit', 'Edit')}
          </Button>
          <Button variant="outline" size="sm" onClick={() => setShowDelete(true)}>
            <Trash2 className="h-3 w-3 mr-1" />
            {t('common.delete', 'Delete')}
          </Button>
        </div>
      </div>

      {/* Description */}
      {project.description && (
        <p className="text-muted-foreground text-sm mt-1">{project.description}</p>
      )}

      {/* Tabs */}
      <Tabs defaultValue="details" className="space-y-4">
        <TabsList>
          <TabsTrigger value="details">{t('projects.tabs.details', 'Details')}</TabsTrigger>
          <TabsTrigger value="paths">{t('projects.tabs.paths', 'Paths')}</TabsTrigger>
          <TabsTrigger value="memories">{t('projects.tabs.memories', 'Memories')}</TabsTrigger>
          <TabsTrigger value="activity">{t('projects.tabs.activity', 'Activity')}</TabsTrigger>
        </TabsList>

        <TabsContent value="details">
          <ProjectDetailsCard project={project} />
        </TabsContent>

        <TabsContent value="paths">
          <ProjectPathsCard project={project} />
        </TabsContent>

        <TabsContent value="memories">
          <ProjectMemoriesCard project={project} />
        </TabsContent>

        <TabsContent value="activity">
          <ProjectActivityCard />
        </TabsContent>
      </Tabs>

      {/* Dialogs */}
      <EditProjectDialog
        project={project}
        open={showEdit}
        onOpenChange={setShowEdit}
        onSuccess={() => {
          setShowEdit(false)
          refetch()
        }}
      />
      <DeleteProjectDialog
        project={project}
        open={showDelete}
        onOpenChange={setShowDelete}
      />
    </div>
  )
}