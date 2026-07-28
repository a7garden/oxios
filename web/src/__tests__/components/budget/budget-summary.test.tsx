import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'
import { BudgetSummaryCard } from '@/components/budget/budget-summary'
import type { BudgetSummary } from '@/types/budget'

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: 'en' },
  }),
}))

describe('BudgetSummaryCard', () => {
  it('renders total agents, tokens used/limit with primary-colored bar at low usage', () => {
    const summary: BudgetSummary = {
      total_agents: 3,
      total_tokens_used: 50_000,
      total_tokens_limit: 200_000,
      exhausted_agents: 0,
    }

    const { container } = render(<BudgetSummaryCard summary={summary} />)

    // Total Agents count is rendered as the consumer-visible value.
    expect(screen.getByText('3')).toBeInTheDocument()

    // Format-through-component: tokens used appears as "50,000" (toLocaleString)
    // and the limit as a sibling span "/ 200,000".
    expect(screen.getByText('50,000')).toBeInTheDocument()
    expect(screen.getByText('/ 200,000')).toBeInTheDocument()

    // 25% usage → primary (low traffic) bar. Width is a derived percent,
    // asserted via the rendered style attribute — NOT recomputed in the test.
    const bar = container.querySelector('div[style*="width"]') as HTMLElement | null
    expect(bar).not.toBeNull()
    expect(bar!.style.width).toBe('25%')
    expect(bar!.className).toContain('bg-primary')
    expect(bar!.className).not.toContain('bg-warning')
    expect(bar!.className).not.toContain('bg-error')
  })

  it('renders the bar in warning color between 70% and 90%', () => {
    const summary: BudgetSummary = {
      total_agents: 2,
      total_tokens_used: 150_000,
      total_tokens_limit: 200_000, // 75%
      exhausted_agents: 0,
    }

    const { container } = render(<BudgetSummaryCard summary={summary} />)

    expect(screen.getByText('150,000')).toBeInTheDocument()
    expect(screen.getByText('/ 200,000')).toBeInTheDocument()

    const bar = container.querySelector('div[style*="width"]') as HTMLElement | null
    expect(bar).not.toBeNull()
    expect(bar!.style.width).toBe('75%')
    expect(bar!.className).toContain('bg-warning')
    expect(bar!.className).not.toContain('bg-primary')
    expect(bar!.className).not.toContain('bg-error')
  })

  it('renders the bar in error color at 90%+ and the exhausted count in error', () => {
    const summary: BudgetSummary = {
      total_agents: 4,
      total_tokens_used: 190_000,
      total_tokens_limit: 200_000, // 95% → error
      exhausted_agents: 2,
    }

    const { container } = render(<BudgetSummaryCard summary={summary} />)

    expect(screen.getByText('190,000')).toBeInTheDocument()
    expect(screen.getByText('4')).toBeInTheDocument()
    expect(screen.getByText('2')).toBeInTheDocument()

    const bar = container.querySelector('div[style*="width"]') as HTMLElement | null
    expect(bar).not.toBeNull()
    expect(bar!.style.width).toBe('95%')
    expect(bar!.className).toContain('bg-error')

    // The exhausted count gets the text-error class only when > 0.
    const exhausted = screen.getByText('2')
    expect(exhausted.className).toContain('text-error')
  })

  it('renders the bar in error color when the limit is fully exhausted (100%)', () => {
    const summary: BudgetSummary = {
      total_agents: 1,
      total_tokens_used: 100_000,
      total_tokens_limit: 100_000,
      exhausted_agents: 1,
    }

    const { container } = render(<BudgetSummaryCard summary={summary} />)

    const bar = container.querySelector('div[style*="width"]') as HTMLElement | null
    expect(bar).not.toBeNull()
    // Math.min clamps to 100.
    expect(bar!.style.width).toBe('100%')
    expect(bar!.className).toContain('bg-error')
  })

  it('renders a zero-width bar and primary color when total_tokens_limit is 0', () => {
    const summary: BudgetSummary = {
      total_agents: 0,
      total_tokens_used: 0,
      total_tokens_limit: 0,
      exhausted_agents: 0,
    }

    const { container } = render(<BudgetSummaryCard summary={summary} />)

    // The summary shows all four numeric fields as 0. Assert through the
    // count text node (`0` for `total_agents`) so we pin the consumer-
    // visible value without relying on `getByText`, which would be
    // ambiguous against the other zero-value fields.
    const totalAgents = screen.getByText('budget.totalAgents').nextElementSibling as HTMLElement | null
    expect(totalAgents?.textContent).toBe('0')
    const exhausted = screen.getByText('budget.exhaustedCount').nextElementSibling as HTMLElement | null
    expect(exhausted?.textContent).toBe('0')
    const bar = container.querySelector('div[style*="width"]') as HTMLElement | null
    expect(bar).not.toBeNull()
    expect(bar!.style.width).toBe('0%')
    expect(bar!.className).toContain('bg-primary')
  })
})
