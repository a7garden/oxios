import {
  Bot,
  Brain,
  Cpu,
  Database,
  Eye,
  Globe,
  MessageSquare,
  Monitor,
  Send,
  Server,
  Shield,
  Sparkles,
  Terminal,
  Timer,
  Zap,
} from 'lucide-react'
import type { SectionIconKey } from './field-defs'

const ICON_MAP: Record<SectionIconKey, React.ComponentType<{ className?: string }>> = {
  engine: Bot,
  kernel: Cpu,
  exec: Terminal,
  security: Shield,
  scheduler: Timer,
  orchestrator: Zap,
  context: Brain,
  gateway: Globe,
  session: Monitor,
  logging: Server,
  memory: Database,
  channels: Send,
  audit: Eye,
  update: Sparkles,
}

/** Generic fallback used when an id is unknown. */
const Fallback = MessageSquare

interface SectionIconProps {
  iconKey: SectionIconKey | string
  className?: string
}

/**
 * Renders the Lucide icon mapped to a section's `iconKey`. Centralised
 * here so the rail, the section tabs, and the section card all share
 * the same icon vocabulary.
 */
export function SectionIcon({ iconKey, className }: SectionIconProps) {
  const Icon = ICON_MAP[iconKey as SectionIconKey] ?? Fallback
  return <Icon className={className} />
}
