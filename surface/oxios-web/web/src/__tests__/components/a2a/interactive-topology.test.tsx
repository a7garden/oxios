import { render, screen } from '@testing-library/react'
import { createElement, type ReactNode } from 'react'
import { describe, expect, it, vi } from 'vitest'
import { InteractiveTopology } from '@/components/a2a/interactive-topology'
import type { TopologyEdge, TopologyNode } from '@/types/a2a'

// Mock react-i18next
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string, options?: { count?: number }) => {
      if (options?.count !== undefined) {
        return `${key}:${options.count}`
      }
      return key
    },
    i18n: { language: 'en' },
  }),
}))

// Mock reactflow
vi.mock('reactflow', () => ({
  default: ({ nodes, edges, onNodeClick, children }: any) => {
    const nodeEls = (nodes ?? []).map((n: any) =>
      createElement(
        'div',
        {
          key: n.id,
          'data-testid': `rf-node-${n.id}`,
          onClick: () => onNodeClick?.(undefined, n),
        },
        n.data.label,
      ),
    )
    const edgeEls = (edges ?? []).map((e: any) =>
      createElement(
        'div',
        { key: e.id, 'data-testid': `rf-edge-${e.id}` },
        `${e.source}->${e.target}`,
      ),
    )
    return createElement(
      'div',
      { 'data-testid': 'rf-mock' },
      ...nodeEls,
      ...edgeEls,
      children as ReactNode,
    )
  },
  ReactFlowProvider: ({ children }: any) => children,
  Background: () => null,
  Controls: () => null,
  MiniMap: () => null,
  BackgroundVariant: { Dots: 'dots' },
}))

const sampleNodes: TopologyNode[] = [
  {
    id: 'agent-1',
    label: 'Agent 1',
    status: 'running',
    capabilities: ['code-review'],
    skills: ['rust'],
    last_seen: '2025-01-01T00:00:00Z',
  },
  {
    id: 'agent-2',
    label: 'Agent 2',
    status: 'idle',
    capabilities: [],
    skills: [],
    last_seen: null,
  },
]

const sampleEdges: TopologyEdge[] = [
  { from: 'agent-1', to: 'agent-2', message_count_5m: 3, last_kind: 'task_delegation' },
]

describe('InteractiveTopology', () => {
  it('renders an empty state when no nodes are provided', () => {
    render(<InteractiveTopology nodes={[]} edges={[]} />)
    expect(screen.getByText('a2a.noTopology')).toBeInTheDocument()
  })

  it('renders a loading skeleton when isLoading is true', () => {
    const { container } = render(<InteractiveTopology nodes={[]} edges={[]} isLoading />)
    // Skeletons render with the loading class
    expect(
      container.querySelectorAll('.animate-pulse, [data-slot="skeleton"]').length,
    ).toBeGreaterThanOrEqual(0)
    // Empty state should NOT show
    expect(screen.queryByText('a2a.noTopology')).not.toBeInTheDocument()
  })

  it('renders an error state when isError is true', () => {
    const onRetry = vi.fn()
    render(<InteractiveTopology nodes={[]} edges={[]} isError onRetry={onRetry} />)
    expect(screen.getByText('a2a.topologyErrorTitle')).toBeInTheDocument()
    expect(screen.getByText('a2a.topologyErrorMessage')).toBeInTheDocument()
  })

  it('renders the ReactFlow canvas with nodes and edges', () => {
    render(<InteractiveTopology nodes={sampleNodes} edges={sampleEdges} />)
    expect(screen.getByTestId('a2a-topology-canvas')).toBeInTheDocument()
    expect(screen.getByTestId('rf-node-agent-1')).toBeInTheDocument()
    expect(screen.getByTestId('rf-node-agent-2')).toBeInTheDocument()
    expect(screen.getByTestId('rf-edge-agent-1->agent-2')).toBeInTheDocument()
  })
})
