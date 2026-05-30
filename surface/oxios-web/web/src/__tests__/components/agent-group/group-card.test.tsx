import { describe, expect, it } from 'vitest'
import { render, screen } from '@testing-library/react'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Badge } from '@/components/ui/badge'
import { StatusIndicator } from '@/components/shared/status-indicator'

// Mock i18next
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: 'en' },
  }),
}))

describe('GroupCard - Rendering patterns', () => {
  it('renders card with Running status', () => {
    const groupName = 'test-group'
    const status = 'Running'
    const agentsCount = 5

    render(
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center justify-between">
            <span>{groupName}</span>
            <Badge variant="default">
              <StatusIndicator status={status} />
            </Badge>
          </CardTitle>
        </CardHeader>
        <CardContent>
          <p className="text-sm text-muted-foreground">
            {agentsCount} agents
          </p>
        </CardContent>
      </Card>
    )

    expect(screen.getByText(groupName)).toBeInTheDocument()
    expect(screen.getByText(/Running/)).toBeInTheDocument()
    expect(screen.getByText(/5 agents/)).toBeInTheDocument()
  })

  it('renders card with Completed status', () => {
    const groupName = 'completed-group'
    const status = 'Completed'
    const agentsCount = 3

    render(
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center justify-between">
            <span>{groupName}</span>
            <Badge variant="secondary">
              <StatusIndicator status={status} />
            </Badge>
          </CardTitle>
        </CardHeader>
        <CardContent>
          <p className="text-sm text-muted-foreground">
            {agentsCount} agents
          </p>
        </CardContent>
      </Card>
    )

    expect(screen.getByText(groupName)).toBeInTheDocument()
    expect(screen.getByText(/Completed/)).toBeInTheDocument()
    expect(screen.getByText(/3 agents/)).toBeInTheDocument()
  })

  it('renders card with Idle status', () => {
    const groupName = 'idle-group'
    const status = 'Idle'

    render(
      <Card>
        <CardHeader>
          <CardTitle className="flex items-center justify-between">
            <span>{groupName}</span>
            <StatusIndicator status={status} />
          </CardTitle>
        </CardHeader>
      </Card>
    )

    expect(screen.getByText(groupName)).toBeInTheDocument()
    expect(screen.getByText(/Idle/)).toBeInTheDocument()
  })
})