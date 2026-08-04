/**
 * Canvas file node — custom React Flow node for the project canvas.
 *
 * Two visual variants:
 *   - "group" → represents a directory; wider, label-only card.
 *   - "file"  → represents a single file; icon + name + activity badge.
 *
 * When `data.active` is true the node gets a pulsing ring around it so
 * the user can see which files the agent is currently touching.
 */

import {
  File,
  FileCode,
  FileCog,
  FileImage,
  FileJson,
  FileText,
  FileType2,
  Folder,
  type LucideIcon,
} from 'lucide-react'
import { memo } from 'react'
import { Handle, type NodeProps, Position } from 'reactflow'
import { Badge } from '@/components/ui/badge'
import { cn } from '@/lib/utils'

/** Pick a Lucide icon for a file based on its extension. */
function fileIcon(name: string): LucideIcon {
  const ext = name.split('.').pop()?.toLowerCase() ?? ''
  if (['ts', 'tsx', 'js', 'jsx', 'mjs', 'cjs'].includes(ext)) return FileCode
  if (['rs'].includes(ext)) return FileCode
  if (['py', 'pyx', 'pyi'].includes(ext)) return FileCode
  if (['go'].includes(ext)) return FileCode
  if (['json', 'jsonc', 'json5'].includes(ext)) return FileJson
  if (['yaml', 'yml', 'toml', 'xml', 'ini', 'conf', 'config', 'env'].includes(ext)) return FileCog
  if (['md', 'markdown', 'txt', 'rtf'].includes(ext)) return FileText
  if (['html', 'htm', 'css', 'scss', 'sass', 'less'].includes(ext)) return FileType2
  if (['png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp', 'ico', 'svg'].includes(ext)) return FileImage
  return File
}

/** Data attached to a canvas node. */
export interface CanvasNodeData {
  label: string
  variant: 'group' | 'file'
  /** Full filesystem path — used as the React Flow node id and click payload. */
  path: string
  /** True while the agent is reading or writing this file. */
  active: boolean
  /** Pending change action, if any. */
  change: 'create' | 'modify' | 'delete' | null
  /** Click handler — opens file in editor. */
  onOpen: (path: string) => void
}

function CanvasFileNodeInner({ data }: NodeProps<CanvasNodeData>) {
  const { label, variant, active, change, onOpen, path } = data

  // Group (directory) cards are wider and use a folder glyph.
  if (variant === 'group') {
    return (
      <div
        className={cn(
          'group-node flex w-[200px] items-center gap-2 rounded-lg border border-dashed border-border bg-surface-muted px-3 py-2',
        )}
        data-testid="canvas-group-node"
      >
        <Handle type="target" position={Position.Top} className="!h-0 !w-0 !border-0 !opacity-0" />
        <Folder className="h-4 w-4 shrink-0 text-muted-foreground" aria-hidden="true" />
        <span className="truncate text-xs font-medium text-muted-foreground" title={path}>
          {label}
        </span>
        <Handle
          type="source"
          position={Position.Bottom}
          className="!h-0 !w-0 !border-0 !opacity-0"
        />
      </div>
    )
  }

  const Icon = fileIcon(label)

  return (
    <button
      type="button"
      onClick={() => onOpen(path)}
      className={cn(
        'file-node group relative flex w-[170px] cursor-pointer flex-col gap-1 rounded-lg border border-border bg-surface px-3 py-2 text-left shadow-sm',
        'transition-all duration-200 ease-[var(--animate-in-easing)]',
        'hover:border-primary/40 hover:shadow-md',
        'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-surface',
        active && 'ring-2 ring-primary animate-pulse-ring',
      )}
      data-testid="canvas-file-node"
      aria-label={`Open ${label}`}
    >
      <Handle type="target" position={Position.Top} className="!h-0 !w-0 !border-0 !opacity-0" />
      <Handle type="source" position={Position.Bottom} className="!h-0 !w-0 !border-0 !opacity-0" />

      <div className="flex items-center gap-2">
        <Icon className="h-4 w-4 shrink-0 text-primary" aria-hidden="true" />
        <span className="truncate text-xs font-medium" title={path}>
          {label}
        </span>
      </div>

      {(active || change) && (
        <div className="flex items-center gap-1">
          {active && (
            <Badge variant="secondary" className="h-4 px-1.5 py-0 text-[10px] leading-none">
              agent
            </Badge>
          )}
          {change && (
            <Badge
              variant={change === 'delete' ? 'destructive' : 'secondary'}
              className="h-4 px-1.5 py-0 text-[10px] leading-none"
            >
              {change}
            </Badge>
          )}
        </div>
      )}
    </button>
  )
}

export const CanvasFileNode = memo(CanvasFileNodeInner)
