// ChatInputActionBar — compact toolbar between textarea and send button
// Ported from LobeHub's ChatInput ActionBar pattern.
// Controls: web search toggle, knowledge base toggle, file upload

'use client'

import { Globe, GlobeOff, Paperclip, BookOpen, X } from 'lucide-react'
import { type ChangeEvent, useRef } from 'react'
import { cn } from '@/lib/utils'

// ── Types ──

export interface AttachedFile {
  name: string
  size: number
  type: string
  /** Base64 data URL for preview, or file path. */
  dataUrl?: string
}

export interface ChatInputActionBarProps {
  /** Whether web search is enabled for this message. */
  searchEnabled?: boolean
  onToggleSearch?: () => void

  /** Whether knowledge base is attached. */
  knowledgeEnabled?: boolean
  onToggleKnowledge?: () => void

  /** Attached files with preview. */
  attachedFiles?: AttachedFile[]
  onAttachFiles?: (files: FileList) => void
  onRemoveFile?: (index: number) => void

  className?: string
}

// ── Component ──

export function ChatInputActionBar({
  searchEnabled = false,
  onToggleSearch,
  knowledgeEnabled = false,
  onToggleKnowledge,
  attachedFiles = [],
  onAttachFiles,
  onRemoveFile,
  className,
}: ChatInputActionBarProps) {
  const fileInputRef = useRef<HTMLInputElement>(null)

  const handleFileChange = (e: ChangeEvent<HTMLInputElement>) => {
    if (e.target.files && onAttachFiles) {
      onAttachFiles(e.target.files)
      // Reset so same file can be re-selected
      e.target.value = ''
    }
  }

  return (
    <div className={cn('flex flex-col gap-1.5', className)}>
      {/* Attached file chips */}
      {attachedFiles.length > 0 && (
        <div className="flex flex-wrap gap-1 px-1">
          {attachedFiles.map((file, i) => (
            <div
              key={`${file.name}-${i}`}
              className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full bg-muted text-xs"
            >
              <Paperclip className="w-3 h-3 text-muted-foreground" />
              <span className="truncate max-w-[120px]">{file.name}</span>
              {onRemoveFile && (
                <button
                  type="button"
                  onClick={() => onRemoveFile(i)}
                  className="ml-0.5 text-muted-foreground hover:text-foreground"
                >
                  <X className="w-3 h-3" />
                </button>
              )}
            </div>
          ))}
        </div>
      )}

      {/* Action buttons row */}
      <div className="flex items-center gap-0.5 px-1">
        {/* Web search toggle */}
        {onToggleSearch && (
          <button
            type="button"
            onClick={onToggleSearch}
            className={cn(
              'inline-flex items-center gap-1 px-2 py-1 rounded text-xs transition-colors',
              searchEnabled
                ? 'bg-primary/10 text-primary hover:bg-primary/20'
                : 'text-muted-foreground hover:text-foreground hover:bg-muted',
            )}
            title={searchEnabled ? 'Web search on' : 'Web search off'}
          >
            {searchEnabled ? (
              <Globe className="w-3.5 h-3.5" />
            ) : (
              <GlobeOff className="w-3.5 h-3.5" />
            )}
            <span className="hidden sm:inline">Search</span>
          </button>
        )}

        {/* Knowledge base toggle */}
        {onToggleKnowledge && (
          <button
            type="button"
            onClick={onToggleKnowledge}
            className={cn(
              'inline-flex items-center gap-1 px-2 py-1 rounded text-xs transition-colors',
              knowledgeEnabled
                ? 'bg-primary/10 text-primary hover:bg-primary/20'
                : 'text-muted-foreground hover:text-foreground hover:bg-muted',
            )}
            title={knowledgeEnabled ? 'Knowledge base on' : 'Knowledge base off'}
          >
            <BookOpen className="w-3.5 h-3.5" />
            <span className="hidden sm:inline">Knowledge</span>
          </button>
        )}

        {/* File upload */}
        {onAttachFiles && (
          <>
            <input
              ref={fileInputRef}
              type="file"
              multiple
              onChange={handleFileChange}
              className="hidden"
              aria-label="Upload files"
            />
            <button
              type="button"
              onClick={() => fileInputRef.current?.click()}
              className="inline-flex items-center gap-1 px-2 py-1 rounded text-xs text-muted-foreground hover:text-foreground hover:bg-muted transition-colors"
              title="Attach files"
            >
              <Paperclip className="w-3.5 h-3.5" />
              <span className="hidden sm:inline">Files</span>
            </button>
          </>
        )}
      </div>
    </div>
  )
}
