import { useCallback, useRef, useState } from 'react'
import { Upload } from 'lucide-react'
import { api } from '@/lib/api-client'

interface UploadDropZoneProps {
  currentDir: string
  onUploaded: () => void
}

export function UploadDropZone({ currentDir, onUploaded }: UploadDropZoneProps) {
  const [isDragging, setIsDragging] = useState(false)
  const [uploading, setUploading] = useState(false)
  const inputRef = useRef<HTMLInputElement>(null)

  const uploadFile = useCallback(
    async (file: File) => {
      setUploading(true)
      try {
        const text = await file.text()
        const filePath = currentDir ? `${currentDir}/${file.name}` : file.name
        await api.put(`/api/workspace/file/${encodeURIComponent(filePath)}`, text, true)
        onUploaded()
      } catch (err) {
        console.error('Upload failed:', err)
      } finally {
        setUploading(false)
      }
    },
    [currentDir, onUploaded],
  )

  const handleDrop = useCallback(
    (e: React.DragEvent) => {
      e.preventDefault()
      setIsDragging(false)
      const files = Array.from(e.dataTransfer.files)
      for (const file of files) {
        uploadFile(file)
      }
    },
    [uploadFile],
  )

  const handleFileSelect = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      const files = Array.from(e.target.files ?? [])
      for (const file of files) {
        uploadFile(file)
      }
      // Reset so the same file can be re-uploaded
      e.target.value = ''
    },
    [uploadFile],
  )

  return (
    <div
      className={`relative rounded-lg border-2 border-dashed p-4 text-center transition-colors ${
        isDragging
          ? 'border-primary bg-primary/5'
          : 'border-muted-foreground/25 hover:border-muted-foreground/50'
      }`}
      onDragOver={(e) => {
        e.preventDefault()
        setIsDragging(true)
      }}
      onDragLeave={() => setIsDragging(false)}
      onDrop={handleDrop}
    >
      <input
        ref={inputRef}
        type="file"
        className="hidden"
        multiple
        onChange={handleFileSelect}
      />
      <button
        type="button"
        className="flex flex-col items-center gap-1 w-full text-sm text-muted-foreground hover:text-foreground transition-colors"
        onClick={() => inputRef.current?.click()}
        disabled={uploading}
      >
        <Upload className="h-5 w-5" />
        {uploading ? 'Uploading...' : 'Drop files here or click to upload'}
      </button>
    </div>
  )
}
