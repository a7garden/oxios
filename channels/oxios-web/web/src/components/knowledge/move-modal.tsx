import { useState, useEffect, useRef } from 'react'
import { ArrowRightLeft, X } from 'lucide-react'
import { Input } from '@/components/ui/input'

export function MoveModal() {
  const [open, setOpen] = useState(false)
  const inputRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'm') {
        e.preventDefault()
        e.stopPropagation()
        setOpen(true)
      }
      if (e.key === 'Escape' && open) {
        setOpen(false)
      }
    }
    window.addEventListener('keydown', handleKeyDown, true)
    return () => window.removeEventListener('keydown', handleKeyDown, true)
  }, [open])

  useEffect(() => {
    if (open) inputRef.current?.focus()
  }, [open])

  if (!open) return null

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center pt-[30vh]">
      <div className="fixed inset-0 bg-black/50" onClick={() => setOpen(false)} />
      <div className="relative w-full max-w-md bg-background border rounded-lg shadow-lg overflow-hidden">
        <div className="flex items-center gap-2 p-3 border-b">
          <ArrowRightLeft className="h-4 w-4 text-muted-foreground" />
          <Input
            ref={inputRef}
            placeholder="Move to folder..."
            className="border-0 shadow-none focus-visible:ring-0"
          />
          <button type="button" onClick={() => setOpen(false)}>
            <X className="h-4 w-4 text-muted-foreground" />
          </button>
        </div>
        <div className="p-3 text-sm text-muted-foreground">
          Select destination folder
        </div>
      </div>
    </div>
  )
}
