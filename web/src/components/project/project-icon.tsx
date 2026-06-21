import {
  BarChart3,
  BookOpen,
  FileText,
  Gamepad2,
  Globe,
  Lightbulb,
  Lock,
  Package,
  Palette,
  Rocket,
  Target,
  Tent,
  Wrench,
  Zap,
} from 'lucide-react'

/**
 * Map icon name (stored in project.emoji field) to Lucide component type.
 * Falls back to Package for unknown names.
 */
const ICON_MAP: Record<string, React.ComponentType<{ className?: string }>> = {
  package: Package,
  wrench: Wrench,
  'file-text': FileText,
  gamepad: Gamepad2,
  globe: Globe,
  'book-open': BookOpen,
  palette: Palette,
  zap: Zap,
  target: Target,
  rocket: Rocket,
  lightbulb: Lightbulb,
  lock: Lock,
  'bar-chart': BarChart3,
  tent: Tent,
}

/** Get a Lucide icon for a project. Accepts the `emoji` field value (icon name or legacy emoji). */
export function getProjectIcon(emoji?: string | null, className?: string): React.ReactNode {
  if (!emoji) return <Package className={className ?? 'h-5 w-5'} />
  const Icon = ICON_MAP[emoji]
  if (Icon) return <Icon className={className ?? 'h-5 w-5'} />
  // Legacy emoji string — just show Package
  return <Package className={className ?? 'h-5 w-5'} />
}
