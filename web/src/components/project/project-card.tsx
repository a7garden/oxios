import { Link } from '@tanstack/react-router'
import { FolderOpen, Pencil, Trash2 } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { formatRelativeTime } from '@/lib/utils'
import type { Project } from '@/types'
import { getProjectIcon } from './project-icon'

interface ProjectCardProps {
  project: Project
  onEdit: (project: Project) => void
  onDelete: (project: Project) => void
}

const SOURCE_COLORS: Record<string, string> = {
  manual: 'bg-success-subtle text-success',
  auto_detected: 'bg-warning-subtle text-warning',
}

export function ProjectCard({ project, onEdit, onDelete }: ProjectCardProps) {
  const { t } = useTranslation()
  const sourceColor = SOURCE_COLORS[project.source ?? 'manual'] ?? SOURCE_COLORS.manual

  return (
    <div className="group rounded-lg border p-4 hover:bg-accent/30 transition-colors">
      {/* Header */}
      <div className="flex items-start justify-between gap-2 mb-2">
        <div className="flex items-center gap-2 min-w-0">
          {getProjectIcon(project.emoji, 'h-5 w-5')}
          <Link
            to="/projects/$projectId"
            params={{ projectId: project.id }}
            className="font-semibold text-sm truncate hover:text-primary"
          >
            {project.name}
          </Link>
          <span className={`shrink-0 text-2xs px-1.5 py-0.5 rounded ${sourceColor}`}>
            {t(
              project.source === 'auto_detected'
                ? 'projects.sourceAutoDetected'
                : 'projects.sourceManual',
            )}
          </span>
        </div>
        <div className="shrink-0 flex items-center gap-0.5">
          <button
            type="button"
            onClick={() => onEdit(project)}
            className="p-2 rounded-md text-muted-foreground hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            aria-label="Edit project"
          >
            <Pencil className="h-4 w-4" />
          </button>
          <button
            type="button"
            onClick={() => onDelete(project)}
            className="p-2 rounded-md text-muted-foreground hover:bg-muted hover:text-destructive focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
            aria-label="Delete project"
          >
            <Trash2 className="h-4 w-4" />
          </button>
        </div>
      </div>

      {/* Description */}
      {project.description && (
        <p className="text-xs text-muted-foreground mb-2 line-clamp-2">{project.description}</p>
      )}

      {/* Paths */}
      {project.paths && project.paths.length > 0 && (
        <p className="text-2xs text-muted-foreground font-mono truncate mb-2">
          <FolderOpen className="h-3 w-3 shrink-0" /> {project.paths[0]}
          {project.paths.length > 1 ? ` +${project.paths.length - 1}` : ''}
        </p>
      )}

      {/* Tags */}
      {project.tags && project.tags.length > 0 && (
        <div className="flex flex-wrap gap-1 mb-2">
          {project.tags.slice(0, 4).map((tag) => (
            <span
              key={tag}
              className="text-2xs px-1.5 py-0.5 rounded bg-secondary text-secondary-foreground"
            >
              {tag}
            </span>
          ))}
          {project.tags.length > 4 && (
            <span className="text-2xs text-muted-foreground">+{project.tags.length - 4}</span>
          )}
        </div>
      )}

      {/* Footer */}
      <div className="flex items-center justify-between text-2xs text-muted-foreground">
        <span>
          {formatRelativeTime(
            project.last_active_at ?? project.updated_at ?? project.created_at,
            t,
          )}
        </span>
        <Link
          to="/projects/$projectId"
          params={{ projectId: project.id }}
          className="hover:text-primary font-medium"
        >
          View →
        </Link>
      </div>
    </div>
  )
}
