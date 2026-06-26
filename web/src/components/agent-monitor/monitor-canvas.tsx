/**
 * Monitor canvas — React Flow wrapper for the unified agent monitor.
 *
 * Takes merged `MonitorNode[]` + `MonitorEdge[]` (from useAgentMonitor)
 * and renders an interactive graph. Single agent = one node, no edges.
 * Multi-agent delegation = nodes + animated A2A message edges.
 *
 * Click a node → opens the detail panel (onNodeSelect). Edges are
 * color-coded by message type and animated when active.
 */

import { useCallback, useEffect, useMemo, useRef } from 'react'
import { useTranslation } from 'react-i18next'
import ReactFlow, {
  Background,
  BackgroundVariant,
  Controls,
  type Edge,
  MiniMap,
  type Node,
  type NodeMouseHandler,
  type ReactFlowInstance,
  type ReactFlowProps,
  ReactFlowProvider,
} from 'reactflow'
import 'reactflow/dist/style.css'
import { Bot } from 'lucide-react'
import { EmptyState } from '@/components/shared/empty-state'
import { statusColor } from '@/components/shared/status-palette'
import type { MonitorEdge, MonitorNode } from '@/types/agent-monitor'
import { MonitorNode as MonitorNodeComp, type MonitorNodeData } from './monitor-node'

const nodeTypes = { monitor: MonitorNodeComp }

/** Map A2A message type → edge stroke color via CSS custom properties. */
function edgeColor(kind: string): string {
  const map: Record<string, string> = {
    task_delegation: 'var(--color-message-task)',
    status_update: 'var(--color-message-status)',
    result_sharing: 'var(--color-message-result)',
    capability_query: 'var(--color-message-query)',
    handshake: 'var(--color-message-handshake)',
  }
  return map[kind] ?? 'var(--color-message-default)'
}

/** Deterministic grid layout so re-renders don't shuffle nodes. */
function nodePosition(index: number, total: number): { x: number; y: number } {
  if (total <= 1) return { x: 0, y: 0 }
  const cols = Math.min(Math.ceil(Math.sqrt(total)), 4)
  const row = Math.floor(index / cols)
  const col = index % cols
  return { x: col * 260, y: row * 180 }
}

interface CanvasProps {
  nodes: MonitorNode[]
  edges: MonitorEdge[]
  isLoading: boolean
  selectedAgentId: string | null
  onNodeSelect: (agentId: string) => void
  /** Node id (name) keyed map for matching topology edges to canvas nodes. */
  className?: string
}

export function MonitorCanvas({
  nodes,
  edges,
  isLoading,
  selectedAgentId,
  onNodeSelect,
  className,
}: CanvasProps) {
  const { t } = useTranslation()
  const rfInstanceRef = useRef<ReactFlowInstance | null>(null)

  const handleNodeClick: NodeMouseHandler = useCallback(
    (_event, node) => onNodeSelect(node.id),
    [onNodeSelect],
  )

  // Build a name→agentId map for edge resolution.
  const nameToId = useMemo(() => {
    const m = new Map<string, string>()
    for (const n of nodes) m.set(n.name, n.agentId)
    return m
  }, [nodes])

  const flowNodes: Node<MonitorNodeData>[] = useMemo(() => {
    return nodes.map((n, i) => {
      const existing = rfInstanceRef.current?.getNode(n.agentId)
      return {
        id: n.agentId,
        type: 'monitor',
        position: existing?.position ?? nodePosition(i, nodes.length),
        data: {
          ...n,
          selected: n.agentId === selectedAgentId,
          onSelect: onNodeSelect,
        },
      }
    })
  }, [nodes, selectedAgentId, onNodeSelect])

  const flowEdges: Edge[] = useMemo(
    () =>
      edges
        .map((e) => {
          // Resolve edge endpoints by name → agentId.
          const sourceId = nameToId.get(e.from) ?? e.from
          const targetId = nameToId.get(e.to) ?? e.to
          // Skip edges referencing unknown nodes.
          if (!nodes.some((n) => n.agentId === sourceId || n.name === e.from)) return null
          if (!nodes.some((n) => n.agentId === targetId || n.name === e.to)) return null
          const isSelected =
            selectedAgentId != null &&
            (sourceId === selectedAgentId || targetId === selectedAgentId)
          return {
            id: `${sourceId}->${targetId}`,
            source: sourceId,
            target: targetId,
            animated: e.messageCount5m > 0,
            label: e.messageCount5m > 1 ? String(e.messageCount5m) : undefined,
            style: {
              stroke: edgeColor(e.lastKind),
              strokeWidth: isSelected ? 2.5 : 1.5,
              opacity: selectedAgentId ? (isSelected ? 1 : 0.2) : 0.7,
            },
          } as Edge
        })
        .filter((e): e is Edge => e !== null),
    [edges, nameToId, nodes, selectedAgentId],
  )

  // Fit view when node count changes.
  useEffect(() => {
    if (rfInstanceRef.current && flowNodes.length > 0) {
      rfInstanceRef.current.fitView({ padding: 0.25, duration: 300 })
    }
  }, [flowNodes.length])

  const handleInit: ReactFlowProps['onInit'] = useCallback((instance) => {
    rfInstanceRef.current = instance
  }, [])

  if (isLoading) {
    return (
      <div className="flex h-[520px] items-center justify-center rounded-xl border bg-background">
        <div className="h-8 w-8 animate-spin rounded-full border-2 border-primary border-t-transparent" />
      </div>
    )
  }

  if (nodes.length === 0) {
    return (
      <div className="flex h-[520px] items-center justify-center rounded-xl border bg-background">
        <EmptyState
          icon={<Bot className="h-10 w-10" aria-hidden="true" />}
          title={t('agentMonitor.noAgents')}
          description={t('agentMonitor.noAgentsDescription')}
        />
      </div>
    )
  }

  return (
    <div
      className={`h-[calc(100vh-12rem)] min-h-[480px] w-full rounded-xl border bg-background ${className ?? ''}`}
      data-testid="monitor-canvas"
    >
      <ReactFlowProvider>
        <ReactFlow
          nodes={flowNodes}
          edges={flowEdges}
          nodeTypes={nodeTypes}
          fitView
          fitViewOptions={{ padding: 0.25 }}
          onInit={handleInit}
          onNodeClick={handleNodeClick}
          proOptions={{ hideAttribution: true }}
          minZoom={0.2}
          maxZoom={2.5}
          zoomOnPinch
          panOnDrag
          zoomOnScroll
          preventScrolling
          nodesDraggable
          nodesConnectable={false}
          edgesFocusable
          elementsSelectable
        >
          <Background variant={BackgroundVariant.Dots} gap={16} size={1} />
          <Controls
            position="bottom-right"
            showInteractive={false}
            aria-label={t('agentMonitor.graphControls')}
          />
          <MiniMap
            pannable
            zoomable
            nodeStrokeColor={(n) =>
              statusColor(String((n.data as MonitorNodeData | undefined)?.displayStatus ?? 'idle'))
            }
            nodeColor={(n) =>
              statusColor(String((n.data as MonitorNodeData | undefined)?.displayStatus ?? 'idle'))
            }
            maskColor="rgba(0,0,0,0.06)"
            aria-label={t('agentMonitor.graphMinimap')}
          />
        </ReactFlow>
      </ReactFlowProvider>
    </div>
  )
}
