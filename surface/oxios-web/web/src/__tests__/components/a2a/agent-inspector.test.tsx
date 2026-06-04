import { describe, expect, it, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { AgentInspector } from '@/components/a2a/agent-inspector'
import type { A2AAgentCard, A2AMessage, TopologyNode } from '@/types/a2a'

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: 'en' },
  }),
}))

const baseNode: TopologyNode = {
  id: 'agent-1',
  label: 'Agent One',
  status: 'running',
  capabilities: ['code-review'],
  skills: ['rust'],
  last_seen: '2025-01-01T00:00:00Z',
}

const baseCard: A2AAgentCard = {
  agent_id: 'uuid-1',
  name: 'Agent One',
  description: 'A test agent',
  capabilities: ['code-review', 'refactor'],
  skills: ['rust', 'python'],
  status: 'running',
  endpoint: 'local',
}

const baseMessages: A2AMessage[] = [
  {
    request_id: 'r-1',
    from_agent: 'agent-1',
    to_agent: 'agent-2',
    message_type: 'task_delegation',
    payload_summary: 'Review the PR',
    accepted: true,
    timestamp: '2025-01-01T00:00:00Z',
  },
  {
    request_id: 'r-2',
    from_agent: 'agent-2',
    to_agent: 'agent-1',
    message_type: 'result_sharing',
    payload_summary: 'Done',
    accepted: true,
    timestamp: '2025-01-01T00:01:00Z',
  },
]

describe('AgentInspector', () => {
  it('renders nothing when node is null', () => {
    const { container } = render(<AgentInspector node={null} open={false} onClose={() => {}} />)
    expect(container.querySelector('[data-testid="a2a-agent-inspector"]')).toBeNull()
  })

  it('renders the inspector panel with capabilities, skills and messages', () => {
    render(
      <AgentInspector
        node={baseNode}
        open
        onClose={() => {}}
        agentCard={baseCard}
        recentMessages={baseMessages}
      />,
    )

    expect(screen.getByTestId('a2a-agent-inspector')).toBeInTheDocument()
    expect(screen.getByText('Agent One')).toBeInTheDocument()
    expect(screen.getByText('code-review')).toBeInTheDocument()
    expect(screen.getByText('refactor')).toBeInTheDocument()
    expect(screen.getByText('rust')).toBeInTheDocument()
    expect(screen.getByText('python')).toBeInTheDocument()

    const messages = screen.getByTestId('a2a-inspector-messages')
    expect(messages).toBeInTheDocument()
    expect(messages.querySelectorAll('li')).toHaveLength(2)
  })

  it('calls onClose when Escape is pressed', () => {
    const onClose = vi.fn()
    render(
      <AgentInspector
        node={baseNode}
        open
        onClose={onClose}
        agentCard={baseCard}
        recentMessages={baseMessages}
      />,
    )

    fireEvent.keyDown(document, { key: 'Escape' })
    expect(onClose).toHaveBeenCalled()
  })

  it('triggers action handlers on button click', () => {
    const onViewTrace = vi.fn()
    const onStopAgent = vi.fn()
    render(
      <AgentInspector
        node={baseNode}
        open
        onClose={() => {}}
        agentCard={baseCard}
        recentMessages={baseMessages}
        onViewTrace={onViewTrace}
        onStopAgent={onStopAgent}
      />,
    )

    fireEvent.click(screen.getByText('a2a.inspectorViewTrace'))
    expect(onViewTrace).toHaveBeenCalledWith('agent-1')

    fireEvent.click(screen.getByText('a2a.inspectorStopAgent'))
    expect(onStopAgent).toHaveBeenCalledWith('agent-1')
  })

  it('handles missing card and empty messages gracefully', () => {
    render(
      <AgentInspector
        node={baseNode}
        open
        onClose={() => {}}
        agentCard={null}
        recentMessages={[]}
      />,
    )
    expect(screen.getByText('a2a.inspectorNoMessages')).toBeInTheDocument()
  })
})
