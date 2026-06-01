import { useTranslation } from 'react-i18next'
import type { TopologyNode } from '@/types/a2a'

interface Props {
  nodes: TopologyNode[]
}

/** Simple SVG topology graph with circle layout. */
export function TopologyGraph({ nodes }: Props) {
  const { t } = useTranslation()

  if (nodes.length === 0) {
    return (
      <div className="flex items-center justify-center h-64 text-muted-foreground">
        {t('a2a.noTopology')}
      </div>
    )
  }

  // Arrange nodes in a circle
  const cx = 300
  const cy = 200
  const radius = Math.min(150, cx - 30, cy - 30, 50 + nodes.length * 20)
  const positioned = nodes.map((node, i) => {
    const angle = (2 * Math.PI * i) / nodes.length - Math.PI / 2
    return {
      ...node,
      x: cx + radius * Math.cos(angle),
      y: cy + radius * Math.sin(angle),
    }
  })

  const statusColor: Record<string, string> = {
    active: '#22c55e',
    idle: '#eab308',
    stopped: '#ef4444',
    starting: '#3b82f6',
  }

  return (
    <svg viewBox="0 0 600 400" className="w-full max-w-2xl mx-auto">
      {positioned.map((node) => (
        <g key={node.id}>
          <circle
            cx={node.x}
            cy={node.y}
            r={24}
            fill={statusColor[node.status] ?? '#6b7280'}
            opacity={0.2}
            stroke={statusColor[node.status] ?? '#6b7280'}
            strokeWidth={2}
          />
          <text
            x={node.x}
            y={node.y}
            textAnchor="middle"
            dominantBaseline="central"
            className="text-xs fill-foreground"
            fontWeight="600"
          >
            {node.label.length > 10 ? node.label.slice(0, 10) + '…' : node.label}
          </text>
        </g>
      ))}
    </svg>
  )
}
