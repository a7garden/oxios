import { useMutation, useQueryClient } from '@tanstack/react-query'
import { Download, FileUp, Link2, Type } from 'lucide-react'
import { useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { api } from '@/lib/api-client'
import { cn } from '@/lib/utils'
import type { Skill } from '@/types'

interface ImportDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  /** Optional initial mode (opened from a specific menu entry). */
  initialMode?: 'file' | 'text' | 'url'
}

type Mode = 'file' | 'text' | 'url'

/**
 * Skill import modal (design F2) — three modes sharing one dialog:
 *  - file: multipart upload (.md / .zip / .skill) → POST /api/skills/import
 *  - text: pasted SKILL.md (frontmatter preserved) → POST /api/skills/import/text
 *  - url:  fetch a remote SKILL.md → POST /api/skills/import/url
 */
export function ImportDialog({ open, onOpenChange, initialMode = 'file' }: ImportDialogProps) {
  const { t } = useTranslation()
  const qc = useQueryClient()
  const [mode, setMode] = useState<Mode>(initialMode)
  const [file, setFile] = useState<File | null>(null)
  const [dragging, setDragging] = useState(false)
  const [text, setText] = useState('')
  const [url, setUrl] = useState('')
  const [nameOverride, setNameOverride] = useState('')
  const fileInputRef = useRef<HTMLInputElement>(null)

  // Sync mode when the dialog re-opens from a specific menu entry.
  const [lastOpen, setLastOpen] = useState(false)
  if (open !== lastOpen) {
    if (open) setMode(initialMode)
    setLastOpen(open)
  }

  const onSuccess = (_r: unknown, name: string) => {
    toast.success(t('skills.importSuccess', { name }))
    qc.invalidateQueries({ queryKey: ['skills'] })
    onOpenChange(false)
    setFile(null)
    setText('')
    setUrl('')
    setNameOverride('')
  }

  const fileMut = useMutation({
    mutationFn: async (vars: { file: File; name?: string }) => {
      const fd = new FormData()
      fd.append('file', vars.file)
      if (vars.name) fd.append('name', vars.name)
      return api.upload<Skill>('/api/skills/import', fd)
    },
    onSuccess: (r, v) => onSuccess(r, r?.name ?? v.file.name),
    onError: (e: unknown) => toast.error(e instanceof Error ? e.message : t('skills.importFailed')),
  })

  const textMut = useMutation({
    mutationFn: (vars: { content: string; name?: string }) =>
      api.post<Skill>('/api/skills/import/text', vars),
    onSuccess: (r) => onSuccess(r, r?.name ?? 'skill'),
    onError: (e: unknown) => toast.error(e instanceof Error ? e.message : t('skills.importFailed')),
  })

  const urlMut = useMutation({
    mutationFn: (vars: { url: string; name?: string }) =>
      api.post<Skill>('/api/skills/import/url', vars),
    onSuccess: (r) => onSuccess(r, r?.name ?? 'skill'),
    onError: (e: unknown) => toast.error(e instanceof Error ? e.message : t('skills.importFailed')),
  })

  const saving = fileMut.isPending || textMut.isPending || urlMut.isPending

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault()
    setDragging(false)
    const f = e.dataTransfer.files?.[0]
    if (f) setFile(f)
  }

  const handleImport = () => {
    if (mode === 'file') {
      if (!file) return
      fileMut.mutate({ file, name: nameOverride || undefined })
    } else if (mode === 'text') {
      if (!text.trim()) return
      textMut.mutate({ content: text, name: nameOverride || undefined })
    } else {
      if (!url.trim()) return
      urlMut.mutate({ url, name: nameOverride || undefined })
    }
  }

  const canImport =
    (mode === 'file' && !!file) ||
    (mode === 'text' && text.trim().length > 0) ||
    (mode === 'url' && url.trim().length > 0)

  if (!open) return null

  const modes: { key: Mode; label: string; icon: React.ReactNode }[] = [
    { key: 'file', label: t('skills.importFromFile'), icon: <FileUp className="h-3.5 w-3.5" /> },
    { key: 'text', label: t('skills.importFromText'), icon: <Type className="h-3.5 w-3.5" /> },
    { key: 'url', label: t('skills.importFromUrl'), icon: <Link2 className="h-3.5 w-3.5" /> },
  ]

  return (
    <div
      className="fixed inset-0 z-50 bg-black/60 backdrop-blur-sm flex items-center justify-center p-6"
      onClick={() => onOpenChange(false)}
    >
      <div
        className="bg-card border rounded-xl w-full max-w-xl max-h-[92vh] flex flex-col overflow-hidden shadow-lg"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center gap-3 px-6 py-4 border-b">
          <span className="font-mono text-[10px] uppercase tracking-wider text-primary bg-primary/10 px-2 py-0.5 rounded-full">
            import
          </span>
          <h2 className="font-semibold text-lg">{t('skills.import')}</h2>
        </div>

        <div className="p-6 overflow-y-auto flex-1 space-y-4">
          {/* Mode switcher */}
          <div className="inline-flex bg-muted rounded-lg p-1 gap-0.5">
            {modes.map((m) => (
              <button
                key={m.key}
                type="button"
                onClick={() => setMode(m.key)}
                className={cn(
                  'inline-flex items-center gap-1.5 rounded-md px-3 py-1.5 text-xs font-medium transition-all',
                  mode === m.key
                    ? 'bg-background text-foreground shadow'
                    : 'text-muted-foreground hover:text-foreground',
                )}
              >
                {m.icon}
                {m.label}
              </button>
            ))}
          </div>

          {/* Name override (optional, all modes) */}
          <div>
            <label className="block text-xs font-semibold text-muted-foreground mb-1">
              {t('skills.nameLabel')} (optional)
            </label>
            <Input
              className="font-mono text-sm max-w-xs"
              placeholder="my-skill"
              value={nameOverride}
              onChange={(e) => setNameOverride(e.target.value)}
            />
          </div>

          {mode === 'file' && (
            <div
              className={cn(
                'border-2 border-dashed rounded-lg px-6 py-10 text-center cursor-pointer transition-colors',
                dragging ? 'border-primary bg-primary/5' : 'border-input hover:border-primary/50',
              )}
              onClick={() => fileInputRef.current?.click()}
              onDragOver={(e) => {
                e.preventDefault()
                setDragging(true)
              }}
              onDragLeave={() => setDragging(false)}
              onDrop={handleDrop}
            >
              <input
                ref={fileInputRef}
                type="file"
                accept=".md,.zip,.skill"
                className="hidden"
                onChange={(e) => setFile(e.target.files?.[0] ?? null)}
              />
              <FileUp className="h-8 w-8 mx-auto mb-3 text-primary" />
              {file ? (
                <p className="text-sm font-medium">{file.name}</p>
              ) : (
                <>
                  <p className="text-sm font-medium">{t('skills.importDropzone')}</p>
                  <p className="text-xs text-muted-foreground mt-1">
                    {t('skills.importDropzoneSub')}
                  </p>
                </>
              )}
              <div className="flex gap-1.5 justify-center mt-3">
                {['.md', '.zip', '.skill'].map((f) => (
                  <span
                    key={f}
                    className="font-mono text-[10px] bg-muted border px-2 py-0.5 rounded text-muted-foreground"
                  >
                    {f}
                  </span>
                ))}
              </div>
            </div>
          )}

          {mode === 'text' && (
            <textarea
              className="font-mono text-xs w-full bg-background border rounded-md px-3 py-3 outline-none focus:border-ring focus:ring-2 focus:ring-ring/20 min-h-[240px] resize-y"
              spellCheck={false}
              placeholder={'---\nname: my-skill\ndescription: ...\n---\n\n# My Skill'}
              value={text}
              onChange={(e) => setText(e.target.value)}
            />
          )}

          {mode === 'url' && (
            <Input
              className="font-mono text-sm"
              placeholder="https://example.com/skills/my-skill/SKILL.md"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
            />
          )}
        </div>

        <div className="flex items-center gap-2 px-6 py-4 border-t bg-background">
          <div className="flex-1" />
          <Button variant="ghost" onClick={() => onOpenChange(false)} disabled={saving}>
            {t('common.cancel')}
          </Button>
          <Button onClick={handleImport} disabled={!canImport || saving}>
            <Download className="h-4 w-4" />
            {t('skills.import')}
          </Button>
        </div>
      </div>
    </div>
  )
}
