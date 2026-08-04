/**
 * Project canvas — React Flow visualization of the project structure.
 *
 * Replaces the editor area when `useCodeLayoutStore.showCanvas` is true.
 * Renders a hierarchical graph of directories (group nodes) and
 * significant files (file nodes) rooted at the session's `project_path`.
 *
 * - Click a file node → opens the file in the editor.
 * - Files the agent is currently reading/writing get a pulsing ring.
 * - Files in `pendingChanges` show a small badge with the change action.
 * - Built-in MiniMap + Controls + zoom/pan.
 */

import { Folder, Loader2 } from 'lucide-react'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import ReactFlow, {
  Background,
  BackgroundVariant,
  Controls,
  type Edge,
  MarkerType,
  MiniMap,
  type Node,
  type NodeMouseHandler,
  type ReactFlowInstance,
  type ReactFlowProps,
  ReactFlowProvider,
} from 'reactflow'
import { toast } from 'sonner'
import 'reactflow/dist/style.css'

import { EmptyState } from '@/components/shared/empty-state'
import { codeApi } from '@/lib/code-api'
import { useCodeLayoutStore, useCodeSessionStore } from '@/stores/code/code-session'
import type { CodeMessage, DirEntry, FileChange } from '@/types/code'

import { CanvasFileNode, type CanvasNodeData } from './canvas-file-node'
import './project-canvas.css'

const nodeTypes = { canvas: CanvasFileNode }

/* -------------------------------------------------------------------------- */
/*  Tree model                                                                */
/* -------------------------------------------------------------------------- */

/** A directory entry enriched with its loaded children (local-only type). */
interface LoadedEntry extends DirEntry {
  children?: LoadedEntry[]
}

/** Positioned tree node — produced by the layout pass. */
interface PositionedNode {
  entry: LoadedEntry
  depth: number
  /** Number of leaf file descendants — drives the vertical slot count. */
  weight: number
}

/* -------------------------------------------------------------------------- */
/*  Tree loading                                                              */
/* -------------------------------------------------------------------------- */

/** Max recursion depth for the project tree. 2 keeps the canvas readable. */
const MAX_DEPTH = 2
/** Cap on total files loaded so a giant monorepo can't lock up the canvas. */
const MAX_FILES = 40

/** Expand `rootPath` into a small tree, capped by depth + file count. */
async function loadTree(rootPath: string): Promise<LoadedEntry[]> {
  async function walk(
    path: string,
    depth: number,
    fileCounter: { count: number },
  ): Promise<LoadedEntry[]> {
    if (fileCounter.count >= MAX_FILES) return []
    const raw = await codeApi.browse(path)
    // Sort: directories first, alphabetical. Hidden dotfiles last.
    const sorted = [...raw].sort((a, b) => {
      const aDot = a.name.startsWith('.')
      const bDot = b.name.startsWith('.')
      if (aDot !== bDot) return aDot ? 1 : -1
      if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1
      return a.name.localeCompare(b.name)
    })
    const out: LoadedEntry[] = []
    for (const entry of sorted) {
      if (fileCounter.count >= MAX_FILES) break
      if (entry.is_dir) {
        const children = depth < MAX_DEPTH ? await walk(entry.path, depth + 1, fileCounter) : []
        out.push({ ...entry, children })
      } else {
        out.push({ ...entry })
        fileCounter.count += 1
      }
    }
    return out
  }
  return walk(rootPath, 0, { count: 0 })
}

/* -------------------------------------------------------------------------- */
/*  Layout                                                                    */
/* -------------------------------------------------------------------------- */

/** Column width (px) for the top-down tree layout. */
const COL_X = 220
/** Row height (px) for the top-down tree layout. */
const ROW_Y = 96

/** Flatten a tree into positioned rows; directories sort above their children. */
function layoutEntries(entries: LoadedEntry[], depth = 0): PositionedNode[] {
  const positioned: PositionedNode[] = []
  const sorted = [...entries].sort((a, b) => {
    if (a.is_dir !== b.is_dir) return a.is_dir ? -1 : 1
    return a.name.localeCompare(b.name)
  })
  for (const entry of sorted) {
    const node: PositionedNode = { entry, depth, weight: 1 }
    positioned.push(node)
    if (entry.is_dir && entry.children?.length) {
      const childLayout = layoutEntries(entry.children, depth + 1)
      node.weight = childLayout.reduce((sum, p) => sum + p.weight, 1)
      positioned.push(...childLayout)
    }
  }
  return positioned
}

/* -------------------------------------------------------------------------- */
/*  Pending-change helpers                                                    */
/* -------------------------------------------------------------------------- */

/** Reduce `pendingChanges` → a path-keyed map of action. */
function buildChangeMap(changes: FileChange[]): Map<string, FileChange['action']> {
  const map = new Map<string, FileChange['action']>()
  for (const c of changes) map.set(c.path, c.action)
  return map
}

/** True if the most recent assistant tool call is touching `path` (no result yet). */
function isAgentTouching(messages: CodeMessage[], path: string): boolean {
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    const msg = messages[i]
    if (msg?.role !== 'assistant' || !msg.tool_calls) continue
    for (const tc of msg.tool_calls) {
      const args = tc.args as { path?: string; file_path?: string }
      const touched = args?.path === path || args?.file_path === path
      if (touched && tc.result_summary == null) return true
    }
    // Only the most recent assistant message matters for "now".
    return false
  }
  return false
}

/* -------------------------------------------------------------------------- */
/*  Component                                                                 */
/* -------------------------------------------------------------------------- */

/**
 * The full-screen project canvas. Mounts inside the editor slot of the
 * workspace layout. Intended to be the only thing rendered when
 * `useCodeLayoutStore.showCanvas` is true.
 */
export function ProjectCanvas() {
  return (
    <ReactFlowProvider>
      <ProjectCanvasInner />
    </ReactFlowProvider>
  )
}

function ProjectCanvasInner() {
  const session = useCodeSessionStore((s) => s.session)
  const tabs = useCodeSessionStore((s) => s.tabs)
  const addTab = useCodeSessionStore((s) => s.addTab)
  const messages = useCodeSessionStore((s) => s.messages)
  const pendingChanges = useCodeSessionStore((s) => s.pendingChanges)
  const isAgentRunning = useCodeSessionStore((s) => s.isAgentRunning)
  const showCanvas = useCodeLayoutStore((s) => s.showCanvas)

  const [tree, setTree] = useState<LoadedEntry[] | null>(null)
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const rfInstanceRef = useRef<ReactFlowInstance | null>(null)

  const rootPath = session?.project_path ?? null
  const changeMap = useMemo(() => buildChangeMap(pendingChanges), [pendingChanges])

  /** Open a file: read via API, then push a new tab. */
  const openFile = useCallback(
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
          // Dismiss canvas so the editor becomes visible.
          if (showCanvas) useCodeLayoutStore.getState().toggleCanvas()
        } catch (err) {
          const msg = err instanceof Error ? err.message : 'Failed to open file'
          toast.error(msg)
        }
      })()
    },
    [addTab, showCanvas],
  )

  /* ----------------------- load the project tree ----------------------- */

  useEffect(() => {
    if (!rootPath) {
      setTree(null)
      return
    }
    let cancelled = false
    setLoading(true)
    setError(null)
    loadTree(rootPath)
      .then((loaded) => {
        if (cancelled) return
        setTree(loaded)
      })
      .catch((err: unknown) => {
        if (cancelled) return
        const msg = err instanceof Error ? err.message : 'Failed to browse project'
        setError(msg)
        toast.error(msg)
      })
      .finally(() => {
        if (!cancelled) setLoading(false)
      })
    return () => {
      cancelled = true
    }
  }, [rootPath])

  /* ----------------------- flatten tree → flow nodes ------------------- */

  const flowNodes: Node<CanvasNodeData>[] = useMemo(() => {
    if (!tree || !rootPath) return []

    // Synthetic root group anchors the layout.
    const projectName = rootPath.split('/').filter(Boolean).pop() || rootPath
    const rootGroup: Node<CanvasNodeData> = {
      id: '__root__',
      type: 'canvas',
      position: { x: 0, y: 0 },
      data: {
        label: projectName,
        variant: 'group',
        path: rootPath,
        active: false,
        change: null,
        onOpen: openFile,
      },
    }

    const positioned = layoutEntries(tree)
    // Track y-offset per depth so siblings don't pile on top of each other.
    const yByDepth = new Map<number, number>()
    const result: Node<CanvasNodeData>[] = [rootGroup]
    for (const { entry, depth, weight } of positioned) {
      const ySlot = yByDepth.get(depth) ?? 0
      yByDepth.set(depth, ySlot + weight)
      const variant: CanvasNodeData['variant'] = entry.is_dir ? 'group' : 'file'
      const change = changeMap.get(entry.path) ?? null
      const active =
        variant === 'file' && isAgentRunning ? isAgentTouching(messages, entry.path) : false
      result.push({
        id: entry.path,
        type: 'canvas',
        position: { x: depth * COL_X, y: ySlot * ROW_Y + ROW_Y },
        data: {
          label: entry.name,
          variant,
          path: entry.path,
          active,
          change,
          onOpen: openFile,
        },
      })
    }
    return result
  }, [tree, rootPath, changeMap, isAgentRunning, messages, openFile])

  /* ----------------------- parent → child edges ------------------------ */

  const flowEdges: Edge[] = useMemo(() => {
    if (!tree || !rootPath) return []
    const edges: Edge[] = []
    // Root group → top-level entries.
    for (const e of tree) {
      edges.push({
        id: `__root__->${e.path}`,
        source: '__root__',
        target: e.path,
        type: 'smoothstep',
        style: { stroke: 'var(--border)', strokeWidth: 1 },
        markerEnd: { type: MarkerType.ArrowClosed, color: 'var(--border)' },
      })
    }
    // Each directory → its children (recursive over the typed tree).
    function walk(items: LoadedEntry[]) {
      for (const parent of items) {
        if (!parent.is_dir || !parent.children?.length) continue
        for (const child of parent.children) {
          edges.push({
            id: `${parent.path}->${child.path}`,
            source: parent.path,
            target: child.path,
            type: 'smoothstep',
            style: { stroke: 'var(--border)', strokeWidth: 1 },
            markerEnd: { type: MarkerType.ArrowClosed, color: 'var(--border)' },
          })
        }
        walk(parent.children)
      }
    }
    walk(tree)
    return edges
  }, [tree, rootPath])

  /* ----------------------- react flow lifecycle ------------------------ */

  const handleInit: ReactFlowProps['onInit'] = useCallback((instance) => {
    rfInstanceRef.current = instance
  }, [])

  // Fit view whenever the node count changes.
  useEffect(() => {
    if (rfInstanceRef.current && flowNodes.length > 0) {
      rfInstanceRef.current.fitView({ padding: 0.2, duration: 300 })
    }
  }, [flowNodes.length])

  const handleNodeClick: NodeMouseHandler = useCallback(
    (_event, node) => {
      const data = node.data as CanvasNodeData
      if (data.variant === 'file') openFile(data.path)
    },
    [openFile],
  )

  /* ----------------------- empty / loading states ---------------------- */

  if (!session || !rootPath) {
    return (
      <div className="flex h-full w-full items-center justify-center bg-surface">
        <EmptyState
          icon={<Folder className="h-10 w-10" aria-hidden="true" />}
          title="No active session"
          description="Open a code session to visualize its structure."
        />
      </div>
    )
  }

  if (loading) {
    return (
      <div className="flex h-full w-full items-center justify-center gap-2 bg-surface text-sm text-muted-foreground">
        <Loader2 className="h-4 w-4 animate-spin" aria-hidden="true" />
        <span>Loading project tree…</span>
      </div>
    )
  }

  if (error) {
    return (
      <div className="flex h-full w-full items-center justify-center bg-surface text-sm text-error">
        {error}
      </div>
    )
  }

  if (!tree || tree.length === 0) {
    return (
      <div className="flex h-full w-full items-center justify-center bg-surface">
        <EmptyState
          icon={<Folder className="h-10 w-10" aria-hidden="true" />}
          title="Empty project"
          description="This project has no files yet."
        />
      </div>
    )
  }

  /* ----------------------- render -------------------------------------- */

  const openTabs = tabs.length
  const totalNodes = flowNodes.length - 1 // exclude synthetic root

  return (
    <section
      className="relative h-full w-full bg-surface"
      data-testid="project-canvas"
      aria-label="Project structure canvas"
    >
      <div className="absolute left-3 top-2 z-10 flex items-center gap-2 rounded-md bg-surface-raised/80 px-2 py-1 text-2xs text-muted-foreground backdrop-blur">
        <span className="font-mono">{rootPath.split('/').filter(Boolean).pop()}</span>
        <span aria-hidden="true">·</span>
        <span>
          {totalNodes} node{totalNodes === 1 ? '' : 's'}
        </span>
        {openTabs > 0 && (
          <>
            <span aria-hidden="true">·</span>
            <span>
              {openTabs} open tab{openTabs === 1 ? '' : 's'}
            </span>
          </>
        )}
      </div>

      <ReactFlow
        nodes={flowNodes}
        edges={flowEdges}
        nodeTypes={nodeTypes}
        fitView
        fitViewOptions={{ padding: 0.2 }}
        onInit={handleInit}
        onNodeClick={handleNodeClick}
        proOptions={{ hideAttribution: true }}
        minZoom={0.2}
        maxZoom={2.5}
        zoomOnScroll
        panOnDrag
        zoomOnPinch
        nodesDraggable
        nodesConnectable={false}
        edgesFocusable={false}
        elementsSelectable
      >
        <Background variant={BackgroundVariant.Dots} gap={16} size={1} />
        <Controls position="bottom-right" showInteractive={false} aria-label="Canvas controls" />
        <MiniMap
          pannable
          zoomable
          nodeStrokeColor={() => 'var(--border-strong)'}
          nodeColor={(n) => {
            const data = n.data as CanvasNodeData | undefined
            if (!data) return 'var(--surface-muted)'
            return data.active ? 'var(--primary)' : 'var(--surface-muted)'
          }}
          maskColor="rgba(0,0,0,0.06)"
          aria-label="Project overview minimap"
        />
      </ReactFlow>
    </section>
  )
}
