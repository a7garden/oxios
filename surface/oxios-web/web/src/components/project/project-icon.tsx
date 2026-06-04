import {
  Package, Wrench, FileText, Gamepad2, Globe, BookOpen, Palette,
  Zap, Target, Rocket, Lightbulb, Lock, BarChart3, Tent,
} from 'lucide-react'

/**
 * Map icon name (stored in project.emoji field) to Lucide component.
 * Falls back to Package for unknown names.
 */
const ICON_MAP: Record<string, React.ReactNode> = {
  package: <Package className="h-5 w-5" />,
  wrench: <Wrench className="h-5 w-5" />,
  'file-text': <FileText className="h-5 w-5" />,
  gamepad: <Gamepad2 className="h-5 w-5" />,
  globe: <Globe className="h-5 w-5" />,
  'book-open': <BookOpen className="h-5 w-5" />,
  palette: <Palette className="h-5 w-5" />,
  zap: <Zap className="h-5 w-5" />,
  target: <Target className="h-5 w-5" />,
  rocket: <Rocket className="h-5 w-5" />,
  lightbulb: <Lightbulb className="h-5 w-5" />,
  lock: <Lock className="h-5 w-5" />,
  'bar-chart': <BarChart3 className="h-5 w-5" />,
  tent: <Tent className="h-5 w-5" />,
}

/** Get a Lucide icon for a project. Accepts the `emoji` field value (icon name or legacy emoji). */
export function getProjectIcon(emoji?: string | null, className?: string): React.ReactNode {
  if (!emoji) return <Package className={className ?? 'h-5 w-5'} />
  const mapped = ICON_MAP[emoji]
  if (mapped) return className ? <Package className={className} /> : mapped
  // Legacy emoji string — just show Package
  return <Package className={className ?? 'h-5 w-5'} />
}
