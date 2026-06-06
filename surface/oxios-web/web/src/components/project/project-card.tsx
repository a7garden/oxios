import { Link } from '@tanstack/react-router'
import { FolderOpen, Pencil, Trash2 } from 'lucide-react'
import type { Project } from '@/types'
import { getProjectIcon } from './project-icon'

interface ProjectCardProps {
  project: Project
  onEdit: (project: Project) => void
  onDelete: (project: Project) => void
}

const SOURCE_COLORS: Record<string, string> = {
  manual: 'bg-emerald-100 text-emerald-700',
  auto_detected: 'bg-amber-100 text-amber-700',
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

export function ProjectCard({ project, onEdit, onDelete }: ProjectCardProps) {
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
            {project.source ?? 'manual'}
          </span>
        </div>
        <div className="shrink-0 flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
          <button
            type="button"
            onClick={() => onEdit(project)}
            className="p-1 rounded hover:bg-muted text-xs text-muted-foreground"
            title="Edit"
          >
            <Pencil className="h-3.5 w-3.5" />
          </button>
          <button
            type="button"
            onClick={() => onDelete(project)}
            className="p-1 rounded hover:bg-muted text-xs text-destructive"
            title="Delete"
          >
            <Trash2 className="h-3.5 w-3.5" />
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
          {formatRelativeTime(project.last_active_at ?? project.updated_at ?? project.created_at)}
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
