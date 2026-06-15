import { useTranslation } from 'react-i18next'
import { useKnowledgeGraph } from '@/hooks/use-knowledge'
import { useKnowledgeStore } from '@/stores/knowledge'

interface LinkGraphProps {
  className?: string
}

export function LinkGraph({ className }: LinkGraphProps) {
  const { t } = useTranslation()
  const { data: graph, isLoading } = useKnowledgeGraph()
  const openFile = useKnowledgeStore((s) => s.openFile)
  const currentFilePath = useKnowledgeStore((s) => s.currentFilePath)

  if (isLoading)
    return <div className="text-xs text-muted-foreground p-2">{t('knowledge.loadingGraph')}</div>
  if (!graph || graph.nodes.length === 0)
    return <div className="text-xs text-muted-foreground p-2">{t('knowledge.noLinksFound')}</div>

  // Simple layout: arrange nodes in a circle
  const nodes = graph.nodes
  const edges = graph.edges
  const cx = 150
  const cy = 150
  const r = 120

  const positions = new Map<string, { x: number; y: number }>()
  nodes.forEach((node, i) => {
    const angle = (2 * Math.PI * i) / nodes.length - Math.PI / 2
    positions.set(node.id, {
      x: cx + r * Math.cos(angle),
      y: cy + r * Math.sin(angle),
    })
  })

  return (
    <svg
      viewBox="0 0 300 300"
      className={className}
      style={{ maxWidth: '100%', touchAction: 'manipulation' }}
    >
      {/* Edges */}
      {edges.map((edge, i) => {
        const from = positions.get(edge.source)
        const to = positions.get(edge.target)
        if (!from || !to) return null
        return (
          <line
            key={`edge-${i}`}
            x1={from.x}
            y1={from.y}
            x2={to.x}
            y2={to.y}
            stroke="currentColor"
            strokeWidth={1}
            opacity={0.3}
          />
        )
      })}
      {/* Nodes */}
      {nodes.map((node) => {
        const pos = positions.get(node.id)
        if (!pos) return null
        const isActive = node.id === currentFilePath
        return (
          <g key={node.id} onClick={() => openFile(node.id)} style={{ cursor: 'pointer' }}>
            <circle
              cx={pos.x}
              cy={pos.y}
              r={isActive ? 9 : 7}
              fill={isActive ? 'var(--primary)' : 'var(--muted-foreground)'}
              opacity={isActive ? 1 : 0.6}
            />
            <circle cx={pos.x} cy={pos.y} r="14" fill="transparent" />
            <text
              x={pos.x}
              y={pos.y + 14}
              textAnchor="middle"
              className="text-[9px] fill-muted-foreground sm:text-[7px]"
            >
              {node.label.length > 12 ? `${node.label.slice(0, 12)}…` : node.label}
            </text>
          </g>
        )
      })}
    </svg>
  )
}
