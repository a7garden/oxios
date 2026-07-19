// CategorizedMemoryBrowser — memory browser with 5-category tabs
// Ported from LobeHub's memory categorization system.
// Tabs: Identity | Activity | Context | Experience | Preference

'use client'

import { useState, useMemo } from 'react'
import {
  UserCircle, Activity, Compass, GraduationCap, Heart,
  type LucideIcon,
} from 'lucide-react'
import { cn } from '@/lib/utils'
import {
  MEMORY_CATEGORY_METADATA,
  type CategorizedMemory,
  type MemoryCategory,
} from '@/types/memory-categories'

// ── Props ──

interface CategorizedMemoryBrowserProps {
  memories: CategorizedMemory[]
  onSelectMemory?: (memory: CategorizedMemory) => void
  onCreateMemory?: (category: MemoryCategory) => void
  className?: string
}

// ── Icon map ──

const ICONS: Record<string, LucideIcon> = {
  UserCircle, Activity, Compass, GraduationCap, Heart,
}

// ── Component ──

export function CategorizedMemoryBrowser({
  memories,
  onSelectMemory,
  onCreateMemory,
  className,
}: CategorizedMemoryBrowserProps) {
  const [activeCategory, setActiveCategory] = useState<MemoryCategory | 'all'>('all')

  const filtered = useMemo(() => {
    if (activeCategory === 'all') return memories
    return memories.filter((m) => m.category === activeCategory)
  }, [memories, activeCategory])

  const counts = useMemo(() => {
    const c: Record<string, number> = {}
    for (const m of memories) {
      c[m.category] = (c[m.category] ?? 0) + 1
    }
    return c
  }, [memories])

  return (
    <div className={cn('flex flex-col h-full', className)}>
      {/* Category tabs */}
      <div className="flex items-center gap-1 border-b px-2 overflow-x-auto">
        <CategoryTab
          label="All"
          count={memories.length}
          active={activeCategory === 'all'}
          onClick={() => setActiveCategory('all')}
        />
        {MEMORY_CATEGORY_METADATA.map((meta) => {
          const Icon = ICONS[meta.icon] ?? UserCircle
          return (
            <CategoryTab
              key={meta.key}
              label={meta.label}
              count={counts[meta.key] ?? 0}
              active={activeCategory === meta.key}
              onClick={() => setActiveCategory(meta.key)}
              icon={<Icon className={cn('w-3.5 h-3.5', meta.color)} />}
            />
          )
        })}
      </div>

      {/* Memory list */}
      <div className="flex-1 overflow-y-auto p-2 space-y-2">
        {filtered.length === 0 ? (
          <EmptyState
            category={activeCategory}
            onCreate={onCreateMemory ? () => onCreateMemory(activeCategory === 'all' ? 'identity' : activeCategory) : undefined}
          />
        ) : (
          filtered.map((memory) => (
            <MemoryCard
              key={memory.id}
              memory={memory}
              onClick={() => onSelectMemory?.(memory)}
            />
          ))
        )}
      </div>
    </div>
  )
}

// ── Category tab ──

function CategoryTab({
  label,
  count,
  active,
  onClick,
  icon,
}: {
  label: string
  count: number
  active: boolean
  onClick: () => void
  icon?: React.ReactNode
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(
        'flex items-center gap-1.5 px-3 py-2 text-sm border-b-2 transition-colors whitespace-nowrap',
        active
          ? 'border-primary text-foreground font-medium'
          : 'border-transparent text-muted-foreground hover:text-foreground',
      )}
    >
      {icon}
      <span>{label}</span>
      <span className={cn(
        'text-xs px-1.5 py-0.5 rounded-full',
        active ? 'bg-primary/10 text-primary' : 'bg-muted text-muted-foreground',
      )}>
        {count}
      </span>
    </button>
  )
}

// ── Memory card ──

function MemoryCard({
  memory,
  onClick,
}: {
  memory: CategorizedMemory
  onClick?: () => void
}) {
  const meta = MEMORY_CATEGORY_METADATA.find((m) => m.key === memory.category)
  const Icon = meta ? (ICONS[meta.icon] ?? UserCircle) : UserCircle

  // Get primary text based on category
  const primaryText = useMemo(() => {
    switch (memory.category) {
      case 'identity':
        return (memory as { description?: string }).description
          ?? (memory as { role?: string }).role
          ?? 'Unnamed identity'
      case 'activity':
        return (memory as { narrative?: string }).narrative ?? 'Unnamed activity'
      case 'context':
        return (memory as { title?: string }).title
          ?? (memory as { description?: string }).description
          ?? 'Unnamed context'
      case 'experience':
        return (memory as { keyLearning?: string }).keyLearning ?? 'Unnamed experience'
      case 'preference':
        return (memory as { topic?: string }).topic ?? 'Unnamed preference'
    }
  }, [memory])

  return (
    <button
      type="button"
      onClick={onClick}
      className="block w-full text-left rounded-lg border bg-card p-3 hover:border-primary/30 hover:shadow-sm transition-all"
    >
      <div className="flex items-start gap-2">
        <Icon className={cn('w-4 h-4 mt-0.5 shrink-0', meta?.color)} />
        <div className="flex-1 min-w-0">
          <p className="text-sm font-medium truncate">{primaryText}</p>
          {memory.tags && memory.tags.length > 0 && (
            <div className="flex flex-wrap gap-1 mt-1">
              {memory.tags.slice(0, 3).map((tag) => (
                <span key={tag} className="text-2xs px-1.5 py-0.5 rounded-full bg-muted text-muted-foreground">
                  {tag}
                </span>
              ))}
            </div>
          )}
          <p className="text-2xs text-muted-foreground/60 mt-1">
            {new Date(memory.updatedAt).toLocaleDateString()}
          </p>
        </div>
      </div>
    </button>
  )
}

// ── Empty state ──

function EmptyState({
  category,
  onCreate,
}: {
  category: MemoryCategory | 'all'
  onCreate?: () => void
}) {
  const meta = category !== 'all'
    ? MEMORY_CATEGORY_METADATA.find((m) => m.key === category)
    : null

  return (
    <div className="flex flex-col items-center justify-center py-12 px-4 text-center">
      {meta && (
        (() => {
          const Icon = ICONS[meta.icon] ?? UserCircle
          return <Icon className={cn('w-8 h-8 mb-2', meta.color)} />
        })()
      )}
      <p className="text-sm text-muted-foreground">
        {meta ? `No ${meta.label.toLowerCase()} memories yet` : 'No memories yet'}
      </p>
      {meta && <p className="text-xs text-muted-foreground/60 mt-1">{meta.description}</p>}
      {onCreate && (
        <button
          type="button"
          onClick={onCreate}
          className="mt-3 px-3 py-1.5 rounded-md bg-primary text-primary-foreground text-xs hover:bg-primary/90 transition-colors"
        >
          Add {meta?.label.toLowerCase() ?? 'memory'}
        </button>
      )}
    </div>
  )
}

// Need useMemo import
