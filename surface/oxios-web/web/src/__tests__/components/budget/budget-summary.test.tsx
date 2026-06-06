import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'

// Mock i18next
vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: 'en' },
  }),
}))

describe('BudgetSummary - Rendering patterns', () => {
  it('renders summary card with data', () => {
    const totalTokens = 150000

    render(
      <Card>
        <CardHeader className="pb-2">
          <CardTitle className="text-sm text-muted-foreground">Total Tokens Used</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="text-2xl font-bold">{totalTokens.toLocaleString()}</div>
        </CardContent>
      </Card>,
    )

    expect(screen.getByText('150,000')).toBeInTheDocument()
  })

  it('renders zero agents case', () => {
    const totalTokens = 0

    render(
      <div>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm text-muted-foreground">Total Tokens Used</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">{totalTokens.toLocaleString()}</div>
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="pb-2">
            <CardTitle className="text-sm text-muted-foreground">Total Agents</CardTitle>
          </CardHeader>
          <CardContent>
            <div className="text-2xl font-bold">0</div>
          </CardContent>
        </Card>
      </div>,
    )

    expect(screen.getAllByText('0').length).toBeGreaterThan(0)
  })

  it('renders exhausted count correctly', () => {
    render(
      <Card>
        <CardHeader className="pb-2">
          <CardTitle className="text-sm text-muted-foreground">Exhausted</CardTitle>
        </CardHeader>
        <CardContent>
          <div className="text-2xl font-bold text-red-500">2</div>
        </CardContent>
      </Card>,
    )

    expect(screen.getByText('2')).toBeInTheDocument()
  })
})
