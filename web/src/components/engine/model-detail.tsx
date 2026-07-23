// ModelDetail — compact info card for a model's capabilities + pricing
// Shown on click of an info icon in the model picker.

import { Brain, DollarSign, Eye, Maximize, Zap } from 'lucide-react'
import { cn } from '@/lib/utils'
import type { ModelInfo } from '@/types/engine'

// ── Props ──

interface ModelDetailProps {
  model: ModelInfo
  className?: string
}

// ── Component ──

export function ModelDetail({ model, className }: ModelDetailProps) {
  return (
    <div className={cn('space-y-3 text-sm', className)}>
      {/* Header */}
      <div>
        <h4 className="font-semibold">{model.name}</h4>
        <p className="text-xs text-muted-foreground font-mono">{model.id}</p>
      </div>

      {/* Capabilities */}
      <div className="space-y-1.5">
        <h5 className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
          Capabilities
        </h5>
        <div className="flex flex-wrap gap-1.5">
          {model.reasoning && (
            <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-amber-500/10 text-amber-600 dark:text-amber-400 text-xs">
              <Brain className="w-3 h-3" />
              Reasoning
            </span>
          )}
          {model.input.includes('image') && (
            <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-blue-500/10 text-blue-600 dark:text-blue-400 text-xs">
              <Eye className="w-3 h-3" />
              Vision
            </span>
          )}
        </div>
      </div>

      {/* Context & output */}
      <div className="space-y-1.5">
        <h5 className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
          Limits
        </h5>
        <div className="grid grid-cols-2 gap-2">
          <div className="flex items-center gap-1.5 text-xs">
            <Maximize className="w-3 h-3 text-muted-foreground" />
            <span>
              <span className="font-medium tabular-nums">{formatTokens(model.contextWindow)}</span>
              <span className="text-muted-foreground ml-0.5">context</span>
            </span>
          </div>
          <div className="flex items-center gap-1.5 text-xs">
            <Zap className="w-3 h-3 text-muted-foreground" />
            <span>
              <span className="font-medium tabular-nums">{formatTokens(model.maxTokens)}</span>
              <span className="text-muted-foreground ml-0.5">max output</span>
            </span>
          </div>
        </div>
      </div>

      {/* Pricing */}
      <div className="space-y-1.5">
        <h5 className="text-xs font-medium text-muted-foreground uppercase tracking-wide">
          Pricing
        </h5>
        <div className="space-y-1 text-xs">
          <div className="flex items-center gap-1.5">
            <DollarSign className="w-3 h-3 text-muted-foreground" />
            <span className="tabular-nums font-medium">${formatCost(model.costInput)}</span>
            <span className="text-muted-foreground">/ 1M input tokens</span>
          </div>
          <div className="flex items-center gap-1.5">
            <DollarSign className="w-3 h-3 text-muted-foreground" />
            <span className="tabular-nums font-medium">${formatCost(model.costOutput)}</span>
            <span className="text-muted-foreground">/ 1M output tokens</span>
          </div>
          {model.costCacheRead > 0 && (
            <div className="flex items-center gap-1.5">
              <DollarSign className="w-3 h-3 text-muted-foreground" />
              <span className="tabular-nums font-medium">${formatCost(model.costCacheRead)}</span>
              <span className="text-muted-foreground">/ 1M cache read</span>
            </div>
          )}
        </div>
      </div>
    </div>
  )
}

// ── Helpers ──

function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(0)}K`
  return String(n)
}

function formatCost(n: number): string {
  if (n === 0) return '0'
  if (n < 0.01) return n.toFixed(4)
  if (n < 1) return n.toFixed(2)
  return n.toFixed(1)
}
