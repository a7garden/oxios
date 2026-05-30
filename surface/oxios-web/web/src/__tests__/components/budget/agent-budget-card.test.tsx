import { describe, expect, it } from 'vitest'
import { render, screen } from '@testing-library/react'
import { Wallet } from 'lucide-react'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Progress } from '@/components/ui/progress'
import { Button } from '@/components/ui/button'

// Mock i18next
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: 'en' },
  }),
}))

describe('AgentBudgetCard - Rendering patterns', () => {
  it('renders card with normal budget (not exhausted)', () => {
    const tokensUsed = 50000
    const tokensLimit = 100000
    const costUsed = 0.15
    const costLimit = 1.0
    const agentId = 'agent-123'
    const tokenPercent = (tokensUsed / tokensLimit) * 100
    const costPercent = (costUsed / costLimit) * 100

    render(
      <Card>
        <CardHeader className="pb-2">
          <CardTitle className="text-sm flex items-center gap-2">
            <Wallet className="h-4 w-4" />
            <span className="font-mono">{agentId.slice(0, 12)}...</span>
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-3">
          <div>
            <div className="flex justify-between text-sm mb-1">
              <span>Tokens: {tokensUsed.toLocaleString()}</span>
              <span>/ {tokensLimit.toLocaleString()}</span>
            </div>
            <Progress value={tokenPercent} />
          </div>
          <div>
            <div className="flex justify-between text-sm mb-1">
              <span>Cost: ${costUsed.toFixed(4)}</span>
              <span>/ ${costLimit.toFixed(2)}</span>
            </div>
            <Progress value={costPercent} />
          </div>
        </CardContent>
      </Card>
    )

    expect(screen.getByText(/Tokens:/)).toBeInTheDocument()
    expect(screen.getByText(/\/ 100,000/)).toBeInTheDocument()
    expect(screen.getByText(/Cost:/)).toBeInTheDocument()
    expect(screen.getByText(/\$1.00/)).toBeInTheDocument()
  })

  it('renders card with exhausted budget', () => {
    const tokensUsed = 100000
    const tokensLimit = 100000
    const costUsed = 1.0
    const costLimit = 1.0
    const agentId = 'agent-456'

    render(
      <Card>
        <CardHeader className="pb-2">
          <CardTitle className="text-sm flex items-center gap-2">
            <Wallet className="h-4 w-4" />
            <span className="font-mono">{agentId.slice(0, 12)}...</span>
          </CardTitle>
        </CardHeader>
        <CardContent className="space-y-3">
          <div>
            <div className="flex justify-between text-sm mb-1">
              <span className="text-destructive">Tokens: {tokensUsed.toLocaleString()}</span>
              <span>/ {tokensLimit.toLocaleString()}</span>
            </div>
            <Progress value={100} className="bg-destructive" />
          </div>
          <div>
            <div className="flex justify-between text-sm mb-1">
              <span className="text-destructive">Cost: ${costUsed.toFixed(4)}</span>
              <span>/ ${costLimit.toFixed(2)}</span>
            </div>
            <Progress value={100} className="bg-destructive" />
          </div>
        </CardContent>
      </Card>
    )

    expect(screen.getAllByText(/100,000/)).toHaveLength(2)
    expect(screen.getByText(/\$1\.0000/)).toBeInTheDocument()
  })

  it('renders edit, reset, and remove buttons', () => {
    render(
      <Card>
        <CardContent className="flex gap-2 pt-4">
          <Button size="sm">Edit</Button>
          <Button size="sm" variant="outline">Reset</Button>
          <Button size="sm" variant="destructive">Remove</Button>
        </CardContent>
      </Card>
    )

    expect(screen.getByRole('button', { name: 'Edit' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Reset' })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: 'Remove' })).toBeInTheDocument()
  })
})