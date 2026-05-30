import { describe, expect, it } from 'vitest'
import { render, screen } from '@testing-library/react'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { StatusIndicator } from '@/components/shared/status-indicator'
import { EmptyState } from '@/components/shared/empty-state'
import { Bot } from 'lucide-react'

// Mock i18next
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: 'en' },
  }),
}))

describe('AgentCardList - Rendering patterns', () => {
  it('renders agent cards', () => {
    const agents = [
      { id: '1', name: 'Agent Alpha', status: 'Running' },
      { id: '2', name: 'Agent Beta', status: 'Idle' },
    ]

    render(
      <div>
        {agents.map((agent) => (
          <Card key={agent.id}>
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Bot className="h-4 w-4" />
                {agent.name}
              </CardTitle>
            </CardHeader>
            <CardContent>
              <StatusIndicator status={agent.status} />
            </CardContent>
          </Card>
        ))}
      </div>
    )

    expect(screen.getByText('Agent Alpha')).toBeInTheDocument()
    expect(screen.getByText('Agent Beta')).toBeInTheDocument()
    expect(screen.getAllByText(/Running|Idle/)).toHaveLength(2)
  })

  it('renders empty state', () => {
    render(
      <EmptyState
        icon={<Bot className="h-10 w-10" />}
        title="No agents"
        description="No agents found. Start a new agent to see it here."
      />
    )

    expect(screen.getByText('No agents')).toBeInTheDocument()
    expect(screen.getByText(/No agents found/)).toBeInTheDocument()
  })

  it('renders single agent card', () => {
    render(
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Bot className="h-4 w-4" />
            Single Agent
          </CardTitle>
        </CardHeader>
        <CardContent>
          <StatusIndicator status="Running" />
        </CardContent>
      </Card>
    )

    expect(screen.getByText('Single Agent')).toBeInTheDocument()
    expect(screen.getByText('Running')).toBeInTheDocument()
  })
})