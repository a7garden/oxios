/**
 * File Explorer panel for the Code Workspace.
 *
 * Lazy-loaded file tree rooted at the session's `project_path`. Directories
 * expand on click; files open in the editor. Right-click on any entry opens
 * a small context menu with New File / New Folder / Delete actions.
 */
import {
  ChevronRight,
  File,
  FileCode,
  FileCog,
  FileImage,
  FileJson,
  FilePlus,
  FileText,
  FileType2,
  Folder,
  FolderOpen,
  FolderPlus,
  Loader2,
  type LucideIcon,
  RefreshCw,
  Trash2,
} from 'lucide-react'
import { useCallback, useEffect, useMemo, useState } from 'react'
import { toast } from 'sonner'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Input } from '@/components/ui/input'
import { ScrollArea } from '@/components/ui/scroll-area'
import { codeApi } from '@/lib/code-api'
import { cn } from '@/lib/utils'
import { useCodeSessionStore } from '@/stores/code/code-session'
import type { DirEntry } from '@/types/code'

/* -------------------------------------------------------------------------- */
/*  File icon mapping                                                         */
/* -------------------------------------------------------------------------- */

/** Pick a Lucide icon for a file based on its extension. */
function fileIcon(name: string): LucideIcon {
  const ext = name.split('.').pop()?.toLowerCase() ?? ''
  // Code
  if (['ts', 'tsx', 'js', 'jsx', 'mjs', 'cjs'].includes(ext)) return FileCode
  if (['rs'].includes(ext)) return FileCode
  if (['py', 'pyx', 'pyi'].includes(ext)) return FileCode
  if (['go'].includes(ext)) return FileCode
  if (['java', 'kt', 'scala'].includes(ext)) return FileCode
  if (['c', 'h', 'cpp', 'cc', 'hpp'].includes(ext)) return FileCode
  if (['rb'].includes(ext)) return FileCode
  if (['php'].includes(ext)) return FileCode
  if (['swift', 'm'].includes(ext)) return FileCode
  // Data / config
  if (['json', 'jsonc', 'json5'].includes(ext)) return FileJson
  if (['yaml', 'yml', 'toml'].includes(ext)) return FileCog
  if (['xml'].includes(ext)) return FileCog
  if (['ini', 'conf', 'config', 'env'].includes(ext)) return FileCog
  // Text-ish
  if (['md', 'markdown'].includes(ext)) return FileText
  if (['txt', 'rtf'].includes(ext)) return FileText
  if (['html', 'htm', 'css', 'scss', 'sass', 'less'].includes(ext)) return FileType2
  if (['sh', 'bash', 'zsh', 'fish'].includes(ext)) return FileText
  if (['sql'].includes(ext)) return FileText
  if (['log'].includes(ext)) return FileText
  // Images
  if (['png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp', 'ico'].includes(ext)) return FileImage
  if (['svg'].includes(ext)) return FileImage
  return File
}

/* -------------------------------------------------------------------------- */
/*  Tree state                                                                */
/* -------------------------------------------------------------------------- */

interface DirNode {
  children: DirEntry[]
  loaded: boolean
  expanded: boolean
  loading: boolean
  error: string | null
}

type TreeState = Record<string, DirNode>

/* -------------------------------------------------------------------------- */
/*  Sorting                                                                   */
/* -------------------------------------------------------------------------- */

/** Sort directories first, then alphabetically. Hidden dotfiles last. */
function sortEntries(entries: DirEntry[]): DirEntry[] {
  return [...entries].sort((a, b) => {
    const aDot = a.name.startsWith('.')
    const bDot = b.name.startsWith('.')
    if (aDot !== bDot) return aDot ? 1 : -1
    if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1
    return a.name.localeCompare(b.name)
  })
}
/* -------------------------------------------------------------------------- */
/*  Tree node                                                                 */
/* -------------------------------------------------------------------------- */

interface TreeNodeProps {
  entry: DirEntry
  depth: number
  tree: TreeState
  setTree: React.Dispatch<React.SetStateAction<TreeState>>
  activePath: string | null
  onOpenFile: (path: string) => void
  onContextMenu: (entry: DirEntry, x: number, y: number) => void
}

function TreeNode({
  entry,
  depth,
  tree,
  setTree,
  activePath,
  onOpenFile,
  onContextMenu,
}: TreeNodeProps) {
  const node = tree[entry.path]
  const expanded = node?.expanded ?? false
  const loaded = node?.loaded ?? false
  const loading = node?.loading ?? false
  const isActive = !entry.is_dir && activePath === entry.path

  const toggle = useCallback(async () => {
    if (!entry.is_dir) return
    setTree((prev) => {
      const cur = prev[entry.path]
      const willExpand = !(cur?.expanded ?? false)
      // First expand with empty placeholder if not yet loaded.
      const next: TreeState = {
        ...prev,
        [entry.path]: {
          children: cur?.children ?? [],
          loaded: cur?.loaded ?? false,
          expanded: willExpand,
          loading: cur?.loading ?? false,
          error: cur?.error ?? null,
        },
      }
      return next
    })
    // Lazy-load on first expand.
    const cur = tree[entry.path]
    if (!cur?.loaded) {
      setTree((prev) => ({
        ...prev,
        [entry.path]: {
          ...(prev[entry.path] ?? {
            children: [],
            loaded: false,
            expanded: true,
            loading: false,
            error: null,
          }),
          loading: true,
          error: null,
        },
      }))
      try {
        const entries = await codeApi.browse(entry.path)
        setTree((prev) => ({
          ...prev,
          [entry.path]: {
            children: sortEntries(entries),
            loaded: true,
            expanded: true,
            loading: false,
            error: null,
          },
        }))
      } catch (err) {
        const msg = err instanceof Error ? err.message : 'Failed to browse directory'
        setTree((prev) => ({
          ...prev,
          [entry.path]: {
            ...(prev[entry.path] ?? {
              children: [],
              loaded: false,
              expanded: true,
              loading: false,
              error: null,
            }),
            loading: false,
            error: msg,
          },
        }))
        toast.error(msg)
      }
    }
  }, [entry, tree, setTree])

  const handleClick = useCallback(() => {
    if (entry.is_dir) {
      void toggle()
    } else {
      void onOpenFile(entry.path)
    }
  }, [entry, toggle, onOpenFile])

  const handleContextMenu = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault()
      e.stopPropagation()
      onContextMenu(entry, e.clientX, e.clientY)
    },
    [entry, onContextMenu],
  )

  const Icon = entry.is_dir ? (expanded ? FolderOpen : Folder) : fileIcon(entry.name)

  return (
    <div
      role="treeitem"
      aria-expanded={entry.is_dir ? expanded : undefined}
      aria-selected={isActive}
      tabIndex={-1}
    >
      <button
        type="button"
        onClick={handleClick}
        onContextMenu={handleContextMenu}
        className={cn(
          'group flex w-full items-center gap-1 rounded-sm py-1 pr-2 text-left text-xs transition-colors',
          'hover:bg-accent/60',
          isActive && 'bg-primary/10 text-primary hover:bg-primary/15',
        )}
        style={{ paddingLeft: `${depth * 12 + 6}px` }}
      >
        {entry.is_dir ? (
          <ChevronRight
            className={cn(
              'h-3 w-3 shrink-0 text-muted-foreground transition-transform',
              expanded && 'rotate-90',
            )}
          />
        ) : (
          <span className="w-3 shrink-0" aria-hidden />
        )}
        <Icon
          className={cn(
            'h-3.5 w-3.5 shrink-0',
            entry.is_dir ? 'text-muted-foreground' : 'text-muted-foreground',
          )}
        />
        <span className="truncate text-foreground/80">{entry.name}</span>
        {loading && (
          <Loader2 className="ml-auto h-3 w-3 shrink-0 animate-spin text-muted-foreground" />
        )}
      </button>

      {entry.is_dir && expanded && loaded && node && (
        <ul className="m-0 list-none p-0">
          {node.children.length === 0 && (
            <li
              className="py-1 text-[10px] italic text-muted-foreground/60"
              style={{ paddingLeft: `${(depth + 1) * 12 + 6}px` }}
            >
              empty
            </li>
          )}
          {node.children.map((child) => (
            <li key={child.path}>
              <TreeNode
                entry={child}
                depth={depth + 1}
                tree={tree}
                setTree={setTree}
                activePath={activePath}
                onOpenFile={onOpenFile}
                onContextMenu={onContextMenu}
              />
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
/* -------------------------------------------------------------------------- */
/*  Context menu                                                              */
/* -------------------------------------------------------------------------- */

interface ContextMenuState {
  entry: DirEntry
  x: number
  y: number
}

function useContextMenu() {
  const [menu, setMenu] = useState<ContextMenuState | null>(null)

  const open = useCallback((entry: DirEntry, x: number, y: number) => {
    setMenu({ entry, x, y })
  }, [])

  const close = useCallback(() => setMenu(null), [])

  // Dismiss on global Escape or scroll while the menu is open.
  useEffect(() => {
    if (!menu) return
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setMenu(null)
    }
    const onScroll = () => setMenu(null)
    window.addEventListener('keydown', onKey)
    window.addEventListener('scroll', onScroll, true)
    return () => {
      window.removeEventListener('keydown', onKey)
      window.removeEventListener('scroll', onScroll, true)
    }
  }, [menu])

  return { menu, open, close }
}

/* -------------------------------------------------------------------------- */
/*  New-entry dialog                                                          */
/* -------------------------------------------------------------------------- */

type NewEntryKind = 'file' | 'folder'

interface NewEntryDialogProps {
  open: boolean
  kind: NewEntryKind
  parentPath: string
  onOpenChange: (open: boolean) => void
  onCreated: (parentPath: string) => void
}

function NewEntryDialog({ open, kind, parentPath, onOpenChange, onCreated }: NewEntryDialogProps) {
  const [name, setName] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  // Reset state every time the dialog re-opens.
  useEffect(() => {
    if (open) {
      setName('')
      setError(null)
      setBusy(false)
    }
  }, [open])

  const handleSubmit = useCallback(
    async (e: React.FormEvent) => {
      e.preventDefault()
      const trimmed = name.trim()
      if (!trimmed) {
        setError('Name is required')
        return
      }
      if (trimmed.includes('/') || trimmed.includes('\\')) {
        setError('Name cannot contain path separators')
        return
      }
      setBusy(true)
      setError(null)
      try {
        const fullPath = `${parentPath.replace(/\/$/, '')}/${trimmed}`
        await codeApi.createFile(fullPath, kind === 'folder')
        toast.success(kind === 'folder' ? 'Folder created' : 'File created')
        onCreated(parentPath)
        onOpenChange(false)
      } catch (err) {
        const msg = err instanceof Error ? err.message : 'Failed to create entry'
        setError(msg)
        toast.error(msg)
      } finally {
        setBusy(false)
      }
    },
    [name, parentPath, kind, onCreated, onOpenChange],
  )

  const Icon = kind === 'folder' ? FolderPlus : FilePlus
  const title = kind === 'folder' ? 'New Folder' : 'New File'

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Icon className="h-4 w-4" />
            {title}
          </DialogTitle>
          <DialogDescription className="break-all">in {parentPath}</DialogDescription>
        </DialogHeader>
        <form onSubmit={handleSubmit} className="space-y-3">
          <Input
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder={kind === 'folder' ? 'new-folder' : 'untitled.ts'}
            autoFocus
            disabled={busy}
          />
          {error && <p className="text-xs text-destructive">{error}</p>}
          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => onOpenChange(false)}
              disabled={busy}
            >
              Cancel
            </Button>
            <Button type="submit" disabled={busy || !name.trim()}>
              {busy ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : null}
              Create
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}

/* -------------------------------------------------------------------------- */
/*  Delete confirmation dialog                                                */
/* -------------------------------------------------------------------------- */

interface DeleteDialogProps {
  entry: DirEntry | null
  onOpenChange: (open: boolean) => void
  onDeleted: (path: string) => void
}

function DeleteDialog({ entry, onOpenChange, onDeleted }: DeleteDialogProps) {
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const handleDelete = useCallback(async () => {
    if (!entry) return
    setBusy(true)
    setError(null)
    try {
      await codeApi.deleteFile(entry.path)
      toast.success('Deleted')
      onDeleted(entry.path)
      onOpenChange(false)
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Failed to delete'
      setError(msg)
      toast.error(msg)
    } finally {
      setBusy(false)
    }
  }, [entry, onDeleted, onOpenChange])

  return (
    <Dialog open={entry !== null} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-sm">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Trash2 className="h-4 w-4 text-destructive" />
            Delete {entry?.is_dir ? 'folder' : 'file'}?
          </DialogTitle>
          <DialogDescription className="break-all">
            <span className="font-mono text-foreground/80">{entry?.name}</span>
            <br />
            This cannot be undone.
          </DialogDescription>
        </DialogHeader>
        {error && <p className="text-xs text-destructive">{error}</p>}
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)} disabled={busy}>
            Cancel
          </Button>
          <Button variant="destructive" onClick={handleDelete} disabled={busy}>
            {busy ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : null}
            Delete
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}

/* -------------------------------------------------------------------------- */
/*  File explorer                                                             */
/* -------------------------------------------------------------------------- */

export function FileExplorer() {
  const session = useCodeSessionStore((s) => s.session)
  const tabs = useCodeSessionStore((s) => s.tabs)
  const activeTabId = useCodeSessionStore((s) => s.activeTabId)
  const addTab = useCodeSessionStore((s) => s.addTab)

  const [tree, setTree] = useState<TreeState>({})
  const [rootLoading, setRootLoading] = useState(false)
  const [rootError, setRootError] = useState<string | null>(null)
  const { menu, open: openContextMenu, close: closeContextMenu } = useContextMenu()

  const [newEntry, setNewEntry] = useState<{ kind: NewEntryKind; parent: string } | null>(null)
  const [deleteTarget, setDeleteTarget] = useState<DirEntry | null>(null)

  const rootPath = session?.project_path ?? null
  const projectName = useMemo(
    () => (rootPath ? rootPath.split('/').filter(Boolean).pop() || rootPath : 'No session'),
    [rootPath],
  )

  // The path of the file open in the active tab, for highlight.
  const activePath = useMemo(() => {
    if (!activeTabId) return null
    return tabs.find((t) => t.id === activeTabId)?.path ?? null
  }, [tabs, activeTabId])

  const handleOpenFile = useCallback(
    (path: string) => {
      void (async () => {
        try {
          const content = await codeApi.readFile(path)
          addTab({
            id: crypto.randomUUID(),
            path,
            name: path.split('/').pop() || path,
            language: content.language,
            isDirty: false,
            isPreview: false,
            content: content.content,
          })
        } catch (err) {
          toast.error(err instanceof Error ? err.message : 'Failed to open file')
        }
      })()
    },
    [addTab],
  )

  /** (Re)load the root directory. */
  const loadRoot = useCallback(async () => {
    if (!rootPath) return
    setRootLoading(true)
    setRootError(null)
    try {
      const entries = await codeApi.browse(rootPath)
      setTree((prev) => ({
        ...prev,
        [rootPath]: {
          children: sortEntries(entries),
          loaded: true,
          expanded: true,
          loading: false,
          error: null,
        },
      }))
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Failed to browse project'
      setRootError(msg)
      toast.error(msg)
    } finally {
      setRootLoading(false)
    }
  }, [rootPath])

  // Auto-load root when session is set (or changes).
  useEffect(() => {
    if (!rootPath) {
      setTree({})
      return
    }
    // Always re-fetch when rootPath changes (new session).
    if (!tree[rootPath]?.loaded) {
      void loadRoot()
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [rootPath])

  /** After a CRUD mutation on a directory, mark it stale and reload it. */
  const refreshDir = useCallback(async (path: string) => {
    if (!path) return
    try {
      const entries = await codeApi.browse(path)
      setTree((prev) => ({
        ...prev,
        [path]: {
          children: sortEntries(entries),
          loaded: true,
          expanded: prev[path]?.expanded ?? true,
          loading: false,
          error: null,
        },
      }))
    } catch (err) {
      const msg = err instanceof Error ? err.message : 'Failed to refresh directory'
      toast.error(msg)
    }
  }, [])

  const handleNewEntryCreated = useCallback(
    (parentPath: string) => {
      void refreshDir(parentPath)
      // If the parent was already expanded, the new entry appears via refresh.
      // If not, expand it so the user can see what they just created.
      setTree((prev) => {
        const cur = prev[parentPath]
        if (!cur) return prev
        return {
          ...prev,
          [parentPath]: { ...cur, expanded: true },
        }
      })
    },
    [refreshDir],
  )

  const handleDeleted = useCallback(
    (path: string) => {
      // The deleted entry could be a file (we just close the tab if open) or a
      // directory. We refresh every ancestor we know about so the tree stays
      // accurate without walking the filesystem.
      const parents = Object.keys(tree).filter((k) => k !== path && !k.startsWith(`${path}/`))
      setTree((prev) => {
        const next: TreeState = {}
        for (const [k, v] of Object.entries(prev)) {
          if (k === path || k.startsWith(`${path}/`)) continue
          next[k] = v
        }
        return next
      })
      // Close any open tabs inside the deleted path.
      const closeTab = useCodeSessionStore.getState().closeTab
      for (const t of tabs) {
        if (t.path === path || t.path.startsWith(`${path}/`)) closeTab(t.id)
      }
      // Refresh each surviving parent.
      for (const p of parents) void refreshDir(p)
    },
    [refreshDir, tabs, tree],
  )
  /** Build a DirEntry for the project root (for context-menu targets). */
  const rootDirEntry = useMemo<DirEntry>(
    () => ({
      name: projectName,
      path: rootPath ?? '',
      is_dir: true,
      is_file: false,
      size: null,
      modified: null,
    }),
    [projectName, rootPath],
  )

  const openRootContextMenu = useCallback(
    (e: React.MouseEvent) => {
      if (!rootPath) return
      e.preventDefault()
      openContextMenu(rootDirEntry, e.clientX, e.clientY)
    },
    [openContextMenu, rootDirEntry, rootPath],
  )

  const openExplorerContextMenu = useCallback(
    (e: React.MouseEvent) => {
      // Only trigger when right-clicking the outer container itself, not a
      // descendant row (rows handle their own context menu).
      if (e.target === e.currentTarget) openRootContextMenu(e)
    },
    [openRootContextMenu],
  )

  /** Parent path for new-entry creation — directory containing `entry`. */
  const parentPathOf = useCallback((entry: DirEntry): string => {
    if (entry.is_dir) return entry.path
    const idx = entry.path.lastIndexOf('/')
    return idx > 0 ? entry.path.slice(0, idx) : '/'
  }, [])

  /** Clamp menu position so it stays on-screen. */
  const clampedMenuX = menu ? Math.min(menu.x, window.innerWidth - 200) : 0
  const clampedMenuY = menu ? Math.min(menu.y, window.innerHeight - 160) : 0

  const rootChildren = rootPath ? tree[rootPath]?.children : undefined
  const rootLoaded = rootPath ? (tree[rootPath]?.loaded ?? false) : false

  /* ----------------------------- empty states ----------------------------- */

  if (!session || !rootPath) {
    return (
      <div className="flex h-full flex-col bg-surface">
        <div className="flex h-9 items-center border-b px-3 text-xs font-semibold uppercase tracking-wider text-muted-foreground">
          Explorer
        </div>
        <div className="flex flex-1 items-center justify-center p-4 text-center text-xs text-muted-foreground">
          No active session.
        </div>
      </div>
    )
  }

  return (
    <div
      className="flex h-full flex-col bg-surface"
      onClick={closeContextMenu}
      onContextMenu={openExplorerContextMenu}
    >
      {/* Header / breadcrumb */}
      <div
        className="flex h-9 shrink-0 items-center gap-2 border-b px-2 text-xs"
        onContextMenu={openRootContextMenu}
      >
        <Folder className="h-3.5 w-3.5 text-muted-foreground" />
        <span className="truncate font-semibold" title={rootPath}>
          {projectName}
        </span>
        <div className="flex-1" />
        <Button
          variant="ghost"
          size="icon"
          className="h-6 w-6"
          onClick={() => void loadRoot()}
          title="Refresh"
        >
          <RefreshCw className={cn('h-3 w-3', rootLoading && 'animate-spin')} />
        </Button>
      </div>

      <ScrollArea className="flex-1">
        <div className="py-1">
          {rootError && <div className="px-3 py-2 text-xs text-destructive">{rootError}</div>}

          {rootLoading && !rootLoaded && (
            <div className="flex items-center gap-2 px-3 py-2 text-xs text-muted-foreground">
              <Loader2 className="h-3 w-3 animate-spin" />
              Loading…
            </div>
          )}

          {rootLoaded && rootChildren && rootChildren.length === 0 && (
            <div className="px-3 py-4 text-center text-xs italic text-muted-foreground">
              Empty directory
            </div>
          )}

          {rootLoaded && rootChildren && rootChildren.length > 0 && (
            <div role="tree" aria-label="File explorer">
              {rootChildren.map((child) => (
                <TreeNode
                  key={child.path}
                  entry={child}
                  depth={0}
                  tree={tree}
                  setTree={setTree}
                  activePath={activePath}
                  onOpenFile={handleOpenFile}
                  onContextMenu={openContextMenu}
                />
              ))}
            </div>
          )}
        </div>
      </ScrollArea>
      {/* Context menu — native positioned, mirrors `message-context-menu` */}
      {menu && (
        <div
          className="fixed z-50 min-w-[180px] rounded-md border bg-popover p-1 shadow-lg"
          style={{ top: clampedMenuY, left: clampedMenuX }}
          onClick={(e) => e.stopPropagation()}
        >
          <button
            type="button"
            className="flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-xs hover:bg-accent"
            onClick={() => {
              setNewEntry({ kind: 'file', parent: parentPathOf(menu.entry) })
              closeContextMenu()
            }}
          >
            <FilePlus className="h-3.5 w-3.5" />
            New File
          </button>
          <button
            type="button"
            className="flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-xs hover:bg-accent"
            onClick={() => {
              setNewEntry({ kind: 'folder', parent: parentPathOf(menu.entry) })
              closeContextMenu()
            }}
          >
            <FolderPlus className="h-3.5 w-3.5" />
            New Folder
          </button>
          <div className="my-1 h-px bg-border" />
          <button
            type="button"
            className="flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-xs text-destructive hover:bg-destructive/10"
            onClick={() => {
              setDeleteTarget(menu.entry)
              closeContextMenu()
            }}
          >
            <Trash2 className="h-3.5 w-3.5" />
            Delete
          </button>
        </div>
      )}

      {/* New entry dialog */}
      {newEntry && (
        <NewEntryDialog
          open
          kind={newEntry.kind}
          parentPath={newEntry.parent}
          onOpenChange={(o) => !o && setNewEntry(null)}
          onCreated={handleNewEntryCreated}
        />
      )}

      {/* Delete dialog */}
      <DeleteDialog
        entry={deleteTarget}
        onOpenChange={(o) => !o && setDeleteTarget(null)}
        onDeleted={handleDeleted}
      />
    </div>
  )
}
