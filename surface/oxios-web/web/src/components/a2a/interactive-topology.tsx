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
import { Network } from 'lucide-react'
import { EmptyState } from '@/components/shared/empty-state'
import { ErrorState } from '@/components/shared/error-state'
import { LoadingCards } from '@/components/shared/loading'
import { statusColor } from '@/components/shared/status-palette'
import type { TopologyEdge, TopologyNode } from '@/types/a2a'
import { AgentNode, type AgentNodeData } from './agent-node'

interface Props {
  nodes: TopologyNode[]
  edges: TopologyEdge[]
  isLoading?: boolean
  isError?: boolean
  onRetry?: () => void
  onNodeSelect?: (nodeId: string) => void
  selectedNodeId?: string | null
  /** Optional className for the outer wrapper. */
  className?: string
}

// reactflow v11.11.4 chosen for React 19 compat (v12 / @xyflow/react not yet validated).
// See RFC-T1-A §6.
const nodeTypes = { agent: AgentNode }

/**
 * Map message type to an OKLCH edge color via the `--color-message-*`
 * CSS variables defined in `index.css`. Inline styles accept `var(...)`
 * directly, so this stays in sync with the design system's OKLCH tokens
 * (no raw hex, no Tailwind-class dynamic composition needed).
 */
function edgeColor(kind: string): string {
  switch (kind) {
    case 'task_delegation':
      return 'var(--color-message-task)'
    case 'status_update':
      return 'var(--color-message-status)'
    case 'result_sharing':
      return 'var(--color-message-result)'
    case 'capability_query':
      return 'var(--color-message-query)'
    case 'handshake':
      return 'var(--color-message-handshake)'
    default:
      return 'var(--color-message-default)'
  }
}

/**
 * Build a deterministic initial position for a node so that
 * re-renders don't move things around. We use a grid layout.
 */
function initialPosition(index: number, total: number): { x: number; y: number } {
  if (total === 0) return { x: 0, y: 0 }
  const cols = Math.max(1, Math.ceil(Math.sqrt(total)))
  const row = Math.floor(index / cols)
  const col = index % cols
  return { x: 80 + col * 260, y: 60 + row * 160 }
}

/**
 * Interactive A2A topology graph.
 *
 * Wraps React Flow with a custom agent node, controls, minimap and
 * dotted background. Empty/loading/error states are surfaced via the
 * shared components.
 */
export function InteractiveTopology({
  nodes,
  edges,
  isLoading,
  isError,
  onRetry,
  onNodeSelect,
  selectedNodeId,
  className,
}: Props) {
  const { t } = useTranslation()
  const wrapperRef = useRef<HTMLDivElement>(null)
  const rfInstanceRef = useRef<ReactFlowInstance | null>(null)

  const handleNodeClick: NodeMouseHandler = useCallback(
    (_event, node) => {
      onNodeSelect?.(node.id)
    },
    [onNodeSelect],
  )

  const flowNodes: Node<AgentNodeData>[] = useMemo(() => {
    return nodes.map((n, i) => {
      const existing = rfInstanceRef.current?.getNode(n.id)
      return {
        id: n.id,
        type: 'agent',
        position: existing?.position ?? initialPosition(i, nodes.length),
        data: {
          ...n,
          selected: n.id === selectedNodeId,
          onSelect: (id: string) => onNodeSelect?.(id),
        },
      }
    })
  }, [nodes, selectedNodeId, onNodeSelect])

  const flowEdges: Edge[] = useMemo(
    () =>
      edges.map((e) => {
        const isSelected =
          selectedNodeId != null && (e.from === selectedNodeId || e.to === selectedNodeId)
        return {
          id: `${e.from}->${e.to}`,
          source: e.from,
          target: e.to,
          animated: e.message_count_5m > 0,
          label: e.message_count_5m > 1 ? String(e.message_count_5m) : undefined,
          style: {
            stroke: edgeColor(e.last_kind),
            strokeWidth: isSelected ? 2.5 : 1.5,
            opacity: selectedNodeId ? (isSelected ? 1 : 0.25) : 0.75,
          },
        }
      }),
    [edges, selectedNodeId],
  )

  // Fit view whenever the node count changes (e.g. first load).
  useEffect(() => {
    if (rfInstanceRef.current && flowNodes.length > 0) {
      rfInstanceRef.current.fitView({ padding: 0.2, duration: 200 })
    }
  }, [flowNodes.length])

  // Keyboard navigation: focus the first node when the graph mounts.
  const handleInit: ReactFlowProps['onInit'] = useCallback((instance) => {
    rfInstanceRef.current = instance
  }, [])

  if (isLoading) {
    return <LoadingCards count={3} />
  }
  if (isError) {
    return (
      <ErrorState
        title={t('a2a.topologyErrorTitle')}
        message={t('a2a.topologyErrorMessage')}
        onRetry={onRetry}
      />
    )
  }
  if (nodes.length === 0) {
    return (
      <EmptyState
        icon={<Network className="h-10 w-10" aria-hidden="true" />}
        title={t('a2a.noTopology')}
        description={t('a2a.noTopologyDescription')}
      />
    )
  }

  return (
    <div
      ref={wrapperRef}
      className={`h-[520px] w-full rounded-xl border bg-background ${className ?? ''}`}
      data-testid="a2a-topology-canvas"
    >
      <ReactFlowProvider>
        <ReactFlow
          nodes={flowNodes}
          edges={flowEdges}
          nodeTypes={nodeTypes}
          fitView
          fitViewOptions={{ padding: 0.2 }}
          onInit={handleInit}
          onNodeClick={handleNodeClick}
          proOptions={{ hideAttribution: true }}
          minZoom={0.3}
          maxZoom={2}
          nodesDraggable
          nodesConnectable={false}
          edgesFocusable
          elementsSelectable
        >
          <Background variant={BackgroundVariant.Dots} gap={16} size={1} />
          <Controls
            position="bottom-right"
            showInteractive={false}
            aria-label={t('a2a.graphControls')}
          />
          <MiniMap
            pannable
            zoomable
            nodeStrokeColor={(n) => statusColor(String(n.data?.status ?? 'default'))}
            nodeColor={(n) => statusColor(String(n.data?.status ?? 'default'))}
            maskColor="rgba(0,0,0,0.08)"
            aria-label={t('a2a.graphMinimap')}
          />
        </ReactFlow>
      </ReactFlowProvider>
    </div>
  )
}
