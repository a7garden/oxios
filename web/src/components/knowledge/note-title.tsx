/**
 * NoteTitle — inline-editable document title for the knowledge editor toolbar.
 *
 * Displays the note's canonical H1 title (fallback: filename stem). Click to
 * edit inline; Enter or blur commits, Escape cancels.
 *
 * Guards (pre-dispatch, to avoid desynced H1↔filename state — where onSave
 * would write a new H1 but skip the rename because the target collides or the
 * path is protected):
 *  - Protected paths (system files, reserved dirs, non-md) render as a plain
 *    non-editable span — their filenames are load-bearing for other subsystems.
 *  - On commit, the desired rename target is checked against the file tree
 *    BEFORE dispatching `knowledge:set-title`. A collision toasts + reverts;
 *    the H1 never reaches the editor with an unsavable title.
 *
 * The editor (MarkdownEditor) listens for `knowledge:set-title` (path-scoped
 * so a split-view sibling editor is never corrupted), rewrites the H1 line,
 * and saves — the existing onSave rename flow (editor-panel) performs the
 * actual file move + path swap. All rename logic stays in one place.
 */
import { Pencil } from 'lucide-react'
import { useEffect, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { toast } from 'sonner'
import { useKnowledgeFile, useKnowledgeRecursiveTree } from '@/hooks/use-knowledge'
import { desiredRenamePath, extractH1, isProtectedPath } from '@/lib/note-rename'
import { flattenTree } from '@/lib/tree-utils'
import { useKnowledgeStore } from '@/stores/knowledge'

export function NoteTitle() {
  const { t } = useTranslation()
  const currentFilePath = useKnowledgeStore((s) => s.currentFilePath)
  const { data: content } = useKnowledgeFile(currentFilePath)
  const { data: tree } = useKnowledgeRecursiveTree()

  const fileName =
    currentFilePath?.split('/').pop()?.replace(/\.(md|html)$/, '') ?? ''

  // Snapshot of every known file path — used to pre-validate a rename whose
  // target would clobber a different existing note. Deduped with the editor's
  // own recursive-tree fetch (same React Query key).
  const knownPaths = useMemo(() => {
    const set = new Set<string>()
    if (tree) for (const n of flattenTree(tree)) set.add(n.path)
    return set
  }, [tree])

  const isHtml = currentFilePath?.endsWith('.html') ?? false
  const editable = !!currentFilePath && !isProtectedPath(currentFilePath) && !isHtml

  // Display the canonical H1 title (what the user sees in the document body),
  // falling back to the filename stem while content loads or when there is no
  // H1. This is what the user edits — not the sanitized stem.
  const displayTitle = useMemo(() => {
    if (content) {
      const h1 = extractH1(content)
      if (h1) return h1
    }
    return fileName
  }, [content, fileName])

  const [editing, setEditing] = useState(false)
  const [value, setValue] = useState('')
  const inputRef = useRef<HTMLInputElement>(null)

  // Seed + focus the input when entering edit mode. displayTitle is captured
  // at entry time only — a background content refetch must not clobber the
  // user's in-progress edit.
  useEffect(() => {
    if (!editing) return
    setValue(displayTitle)
    const id = requestAnimationFrame(() => {
      inputRef.current?.focus()
      inputRef.current?.select()
    })
    return () => cancelAnimationFrame(id)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [editing])

  // Exit edit mode on file switch.
  useEffect(() => {
    setEditing(false)
  }, [currentFilePath])

  function commit() {
    setEditing(false)
    const trimmed = value.trim()
    if (!currentFilePath || trimmed.length === 0 || trimmed === displayTitle) return

    // Pre-validate the rename target BEFORE dispatching, so an unsavable title
    // never reaches the editor (where onSave would write the new H1 but skip
    // the rename, leaving the doc permanently desynced).
    const target = desiredRenamePath(currentFilePath, trimmed)
    if (target && knownPaths.has(target)) {
      toast.error(t('knowledge.renameCollision'))
      return
    }

    // Safe — the path-scoped editor handler rewrites the H1 line and saves;
    // the existing onSave rename flow (editor-panel) performs the file move.
    document.dispatchEvent(
      new CustomEvent('knowledge:set-title', {
        detail: { path: currentFilePath, title: trimmed },
      }),
    )
  }

  // No file open.
  if (!currentFilePath) {
    return <span className="mx-3 truncate text-sm font-medium">Knowledge</span>
  }

  // Protected or non-markdown — read-only.
  if (!editable) {
    return (
      <span className="mx-3 truncate text-sm font-medium" title={currentFilePath}>
        {fileName || 'Knowledge'}
      </span>
    )
  }

  // Editable — input mode.
  if (editing) {
    return (
      <input
        ref={inputRef}
        value={value}
        aria-label={t('knowledge.titleEditHint')}
        onChange={(e) => setValue(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault()
            commit()
          } else if (e.key === 'Escape') {
            e.preventDefault()
            setEditing(false)
          }
        }}
        style={{ width: `${Math.max(value.length + 1, 8)}ch` }}
        className="mx-2 rounded-md border border-primary bg-background px-2 py-0.5 text-sm font-medium outline-none ring-2 ring-primary/20"
      />
    )
  }

  // Editable — display mode (click to edit). Hover reveals a pencil icon
  // and a subtle background to signal interactivity.
  return (
    <button
      type="button"
      onClick={() => setEditing(true)}
      title={t('knowledge.titleEditHint')}
      className="group mx-2 flex max-w-[40vw] items-center gap-1.5 rounded-md px-2 py-1 text-left text-sm font-medium transition-colors hover:bg-muted cursor-text"
    >
      <span className="truncate">{displayTitle || fileName || 'Knowledge'}</span>
      <Pencil className="h-3 w-3 shrink-0 text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100" />
    </button>
  )
}
