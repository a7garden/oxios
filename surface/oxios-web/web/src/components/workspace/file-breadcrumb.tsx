import { ChevronRight, Home } from 'lucide-react'

interface FileBreadcrumbProps {
  path: string
  onNavigate: (dir: string) => void
}

export function FileBreadcrumb({ path, onNavigate }: FileBreadcrumbProps) {
  if (!path) {
    return (
      <div className="flex items-center gap-1 text-sm text-muted-foreground">
        <Home className="h-4 w-4" />
        <span>root</span>
      </div>
    )
  }

  const segments = path.split('/').filter(Boolean)
  const parts: { label: string; fullPath: string }[] = []
  let accumulated = ''
  for (const seg of segments) {
    accumulated = accumulated ? `${accumulated}/${seg}` : seg
    parts.push({ label: seg, fullPath: accumulated })
  }

  return (
    <div className="flex items-center gap-1 text-sm flex-wrap">
      <button
        type="button"
        className="flex items-center gap-1 text-muted-foreground hover:text-foreground transition-colors"
        onClick={() => onNavigate('')}
      >
        <Home className="h-3.5 w-3.5" />
        <span>root</span>
      </button>
      {parts.map((part, i) => (
        <span key={part.fullPath} className="flex items-center gap-1">
          <ChevronRight className="h-3 w-3 text-muted-foreground" />
          {i === parts.length - 1 ? (
            <span className="font-medium text-foreground">{part.label}</span>
          ) : (
            <button
              type="button"
              className="text-muted-foreground hover:text-foreground transition-colors"
              onClick={() => onNavigate(part.fullPath)}
            >
              {part.label}
            </button>
          )}
        </span>
      ))}
    </div>
  )
}
