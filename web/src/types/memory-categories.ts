// Memory 5-category system (ported from LobeHub userMemory types)
// Categorizes user memory into semantic layers:
// identity, activity, context, experience, preference

// ── Category type ──

export type MemoryCategory = 'identity' | 'activity' | 'context' | 'experience' | 'preference'

export const MEMORY_CATEGORIES: MemoryCategory[] = [
  'identity',
  'activity',
  'context',
  'experience',
  'preference',
]

// ── Category metadata for UI ──

export interface MemoryCategoryMeta {
  key: MemoryCategory
  label: string
  description: string
  icon: string
  color: string
}

export const MEMORY_CATEGORY_METADATA: MemoryCategoryMeta[] = [
  {
    key: 'identity',
    label: 'Identity',
    description: 'Who the user is — role, relationships, demographics',
    icon: 'UserCircle',
    color: 'text-blue-500',
  },
  {
    key: 'activity',
    label: 'Activity',
    description: 'What the user does — events, actions, habits',
    icon: 'Activity',
    color: 'text-emerald-500',
  },
  {
    key: 'context',
    label: 'Context',
    description: 'Situational context — environment, constraints, status',
    icon: 'Compass',
    color: 'text-amber-500',
  },
  {
    key: 'experience',
    label: 'Experience',
    description: 'Past experiences and learnings — heuristics, playbooks',
    icon: 'GraduationCap',
    color: 'text-purple-500',
  },
  {
    key: 'preference',
    label: 'Preference',
    description: "Enduring choices — formats, priorities, do/don't",
    icon: 'Heart',
    color: 'text-rose-500',
  },
]

// ── Base memory item ──

export interface BaseMemoryItem {
  id: string
  category: MemoryCategory
  createdAt: string
  updatedAt: string
  accessedAt?: string
  /** When the event occurred (for episodic memory). */
  episodicDate?: string
  tags?: string[]
}

// ── Identity ──

export type IdentityType = 'personal' | 'professional' | 'demographic'

export interface IdentityMemory extends BaseMemoryItem {
  category: 'identity'
  type?: IdentityType
  role?: string
  relationship?: string
  description?: string
}

// ── Activity ──

export interface ActivityMemory extends BaseMemoryItem {
  category: 'activity'
  narrative?: string
  feedback?: string
  notes?: string
  associatedActions?: string[]
}

// ── Context ──

export interface ContextMemory extends BaseMemoryItem {
  category: 'context'
  title?: string
  description?: string
  currentStatus?: string
  associatedObjects?: string[]
  associatedSubjects?: string[]
}

// ── Experience ──

export interface ExperienceMemory extends BaseMemoryItem {
  category: 'experience'
  action?: string
  situation?: string
  keyLearning?: string
  possibleOutcome?: string
  reasoning?: string
  confidence?: number // 0.0 - 1.0
}

// ── Preference ──

export interface PreferenceMemory extends BaseMemoryItem {
  category: 'preference'
  topic?: string
  conclusionDirectives?: string[] // actionable directives
  suggestions?: string[]
}

// ── Union ──

export type CategorizedMemory =
  | IdentityMemory
  | ActivityMemory
  | ContextMemory
  | ExperienceMemory
  | PreferenceMemory

// ── Persona summary (written by personaWriter model) ──

export interface MemoryPersona {
  /** Free-form persona text written by the personaWriter model. */
  summary: string
  updatedAt: string
  /** Key facts extracted from all memory layers. */
  keyFacts?: string[]
}
