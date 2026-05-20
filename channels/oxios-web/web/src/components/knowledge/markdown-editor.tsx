import { useCallback, useEffect, useRef, useState } from 'react'
import { cn } from '@/lib/utils'

interface MarkdownEditorProps {
  filePath: string
  initialContent: string
  onSave: (content: string) => void
  className?: string
}

export function MarkdownEditor({ filePath, initialContent, onSave, className }: MarkdownEditorProps) {
  const [content, setContent] = useState(initialContent)
  const [isDirty, setIsDirty] = useState(false)
  const textareaRef = useRef<HTMLTextAreaElement>(null)
  const saveTimerRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined)

  // Reset when file changes
  useEffect(() => {
    setContent(initialContent)
    setIsDirty(false)
  }, [filePath, initialContent])

  // Auto-save on blur
  useEffect(() => {
    return () => {
      if (saveTimerRef.current) clearTimeout(saveTimerRef.current)
    }
  }, [])

  const handleChange = useCallback(
    (e: React.ChangeEvent<HTMLTextAreaElement>) => {
      const newContent = e.target.value
      setContent(newContent)
      setIsDirty(true)

      // Debounced auto-save
      if (saveTimerRef.current) clearTimeout(saveTimerRef.current)
      saveTimerRef.current = setTimeout(() => {
        onSave(newContent)
        setIsDirty(false)
      }, 1000)
    },
    [onSave],
  )

  const handleBlur = useCallback(() => {
    if (isDirty) {
      if (saveTimerRef.current) clearTimeout(saveTimerRef.current)
      onSave(content)
      setIsDirty(false)
    }
  }, [isDirty, content, onSave])

  // Keyboard shortcuts
  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      // Cmd/Ctrl+B → bold
      if ((e.metaKey || e.ctrlKey) && e.key === 'b') {
        e.preventDefault()
        wrapSelection('**', '**')
      }
      // Cmd/Ctrl+I → italic
      if ((e.metaKey || e.ctrlKey) && e.key === 'i') {
        e.preventDefault()
        wrapSelection('*', '*')
      }
    },
    [],
  )

  const wrapSelection = (before: string, after: string) => {
    const textarea = textareaRef.current
    if (!textarea) return
    const start = textarea.selectionStart
    const end = textarea.selectionEnd
    const selected = content.substring(start, end)
    const newContent = content.substring(0, start) + before + selected + after + content.substring(end)
    setContent(newContent)
    setIsDirty(true)
    // Restore cursor
    requestAnimationFrame(() => {
      textarea.selectionStart = start + before.length
      textarea.selectionEnd = end + before.length
      textarea.focus()
    })
  }

  return (
    <div className={cn('h-full relative', className)}>
      {isDirty && (
        <span className="absolute top-2 right-3 text-xs text-muted-foreground">Unsaved</span>
      )}
      <textarea
        ref={textareaRef}
        value={content}
        onChange={handleChange}
        onBlur={handleBlur}
        onKeyDown={handleKeyDown}
        className="w-full h-full resize-none p-6 bg-background text-foreground font-mono text-sm leading-relaxed focus:outline-none"
        spellCheck={false}
      />
    </div>
  )
}
