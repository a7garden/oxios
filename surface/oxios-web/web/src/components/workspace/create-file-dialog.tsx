import { useState } from 'react'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'

interface CreateFileDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  currentDir: string
  onSubmit: (name: string, isDir: boolean) => void
}

export function CreateFileDialog({ open, onOpenChange, currentDir, onSubmit }: CreateFileDialogProps) {
  const [name, setName] = useState('')
  const [isDir, setIsDir] = useState(false)

  const handleSubmit = () => {
    const trimmed = name.trim()
    if (!trimmed) return
    const fullPath = currentDir ? `${currentDir}/${trimmed}` : trimmed
    onSubmit(fullPath, isDir)
    setName('')
    setIsDir(false)
    onOpenChange(false)
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      e.preventDefault()
      handleSubmit()
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Create {isDir ? 'Directory' : 'File'}</DialogTitle>
          <DialogDescription>
            {currentDir ? `In: ${currentDir}/` : 'In workspace root'}
          </DialogDescription>
        </DialogHeader>
        <div className="space-y-4 py-2">
          <Input
            value={name}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={isDir ? 'directory-name' : 'filename.ext'}
            autoFocus
          />
          <label className="flex items-center gap-2 text-sm cursor-pointer">
            <input
              type="checkbox"
              checked={isDir}
              onChange={(e) => setIsDir(e.target.checked)}
              className="rounded border-border"
            />
            Create as directory
          </label>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button onClick={handleSubmit} disabled={!name.trim()}>
            Create {isDir ? 'Directory' : 'File'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
