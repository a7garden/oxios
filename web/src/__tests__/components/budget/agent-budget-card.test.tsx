import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'
import { AgentBudgetCard } from '@/components/budget/agent-budget-card'
import type { AgentBudget } from '@/types/budget'

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => key,
    i18n: { language: 'en' },
  }),
}))

// Builders below produce fixtures with deliberately varied width ratios and
// exhaustion states so the assertions can target different branches of the
// production percent presentation (bg-primary < 70, bg-warning 70-89,
// bg-error >= 90) without recomputing the formula in the test.
function makeAgent(overrides: Partial<AgentBudget> = {}): AgentBudget {
  return {
    agent_id: 'a-1234567890abcdef',
    name: 'Agent One',
    has_budget: true,
    budget: {
      token_limit: 100_000,
      tokens_used: 50_000, // 50% → primary
      tokens_remaining: 50_000,
      calls_limit: 100,
      calls_used: 40, // 40% → primary
      calls_remaining: 60,
      window_secs: 3600,
      window_remaining_secs: 1845,
      is_exhausted: false,
    },
    ...overrides,
  }
}

// Five call sites — selector + cast stays lockstep.
function widthBars(container: HTMLElement): HTMLElement[] {
  return Array.from(container.querySelectorAll('div[style*="width"]')) as HTMLElement[]
}

describe('AgentBudgetCard', () => {
  it('renders the agent display name and falls back to a truncated id when no name is set', () => {
    render(
      <AgentBudgetCard
        agent={makeAgent({ name: 'My Favourite Agent' })}
        onEdit={() => {}}
        onReset={() => {}}
        onRemove={() => {}}
        isResetting={false}
        isRemoving={false}
      />,
    )
    expect(screen.getByText('My Favourite Agent')).toBeInTheDocument()

    const { container } = render(
      <AgentBudgetCard
        agent={makeAgent({ agent_id: 'a-1234567890abcdef', name: undefined })}
        onEdit={() => {}}
        onReset={() => {}}
        onRemove={() => {}}
        isResetting={false}
        isRemoving={false}
      />,
    )
    // Fallback path: 12-char slice + ellipsis, derived inside production code.
    expect(container.textContent).toContain('a-1234567890...')
  })

  it('renders tokens and calls formatted through the component with primary bars at low usage', () => {
    const { container } = render(
      <AgentBudgetCard
        agent={makeAgent()}
        onEdit={() => {}}
        onReset={() => {}}
        onRemove={() => {}}
        isResetting={false}
        isRemoving={false}
      />,
    )

    // Production renders `budget.tokens: 50,000` / `budget.calls: 40` and the
    // limit as a sibling span `/ 100,000` / `/ 100`. Assert through
    // container text so we don't depend on the i18n key string in the DOM.
    expect(container.textContent).toContain('50,000')
    expect(container.textContent).toContain('/ 100,000')
    expect(container.textContent).toContain('40')
    expect(container.textContent).toContain('/ 100')
    // Two progress bars: tokens (50%) and calls (40%). Production code uses
    // `bg-primary` for the token bar and `bg-info` for the call bar (their
    // copy and purpose differ); both thresholds for warning/error are > 70.
    const bars = widthBars(container)
    expect(bars).toHaveLength(2)
    const widths = bars.map((b) => b.style.width).sort()
    expect(widths).toEqual(['40%', '50%'])
    const tokenBar = bars.find((b) => b.style.width === '50%')!
    const callBar = bars.find((b) => b.style.width === '40%')!
    expect(tokenBar.className).toContain('bg-primary')
    expect(tokenBar.className).not.toContain('bg-warning')
    expect(tokenBar.className).not.toContain('bg-error')
    expect(callBar.className).toContain('bg-info')
    expect(callBar.className).not.toContain('bg-warning')
    expect(callBar.className).not.toContain('bg-error')
  })

  it('renders a warning-colored token bar at 70-89% and a primary-colored call bar', () => {
    const { container } = render(
      <AgentBudgetCard
        agent={makeAgent({
          budget: {
            token_limit: 100_000,
            tokens_used: 80_000, // 80% → warning
            tokens_remaining: 20_000,
            calls_limit: 100,
            calls_used: 10, // 10% → primary
            calls_remaining: 90,
            window_secs: 3600,
            window_remaining_secs: 1800,
            is_exhausted: false,
          },
        })}
        onEdit={() => {}}
        onReset={() => {}}
        onRemove={() => {}}
        isResetting={false}
        isRemoving={false}
      />,
    )

    const bars = widthBars(container)
    expect(bars).toHaveLength(2)
    const tokenBar = bars.find((b) => b.style.width === '80%')!
    const callBar = bars.find((b) => b.style.width === '10%')!
    expect(tokenBar.className).toContain('bg-warning')
    expect(tokenBar.className).not.toContain('bg-primary')
    expect(callBar.className).toContain('bg-info')
    expect(callBar.className).not.toContain('bg-warning')
  })

  it('applies error color on both bars and shows the exhausted badge + remaining "Exhausted" label when the budget is exhausted', () => {
    const agent = makeAgent({
      budget: {
        token_limit: 100_000,
        tokens_used: 100_000, // 100% → error
        tokens_remaining: 0,
        calls_limit: 100,
        calls_used: 100, // 100% → error
        calls_remaining: 0,
        window_secs: 3600,
        window_remaining_secs: 0,
        is_exhausted: true,
      },
    })

    const { container } = render(
      <AgentBudgetCard
        agent={agent}
        onEdit={() => {}}
        onReset={() => {}}
        onRemove={() => {}}
        isResetting={false}
        isRemoving={false}
      />,
    )

    // Badge surfacing the exhausted state.
    expect(screen.getByText('budget.exhausted')).toBeInTheDocument()

    // Card-level class signals exhausted state to consumers.
    const card = container.querySelector('[class*="border-error"]')
    expect(card).not.toBeNull()

    const bars = widthBars(container)
    expect(bars).toHaveLength(2)
    for (const b of bars) {
      expect(b.style.width).toBe('100%')
      expect(b.className).toContain('bg-error')
    }
  })

  it('clamps the bar width to 100% but still hits the error bucket when tokens_used exceeds the limit', () => {
    const { container } = render(
      <AgentBudgetCard
        agent={makeAgent({
          budget: {
            token_limit: 100_000,
            tokens_used: 250_000, // > limit
            tokens_remaining: 0,
            calls_limit: 100,
            calls_used: 0,
            calls_remaining: 100,
            window_secs: 3600,
            window_remaining_secs: 1200,
            is_exhausted: false,
          },
        })}
        onEdit={() => {}}
        onReset={() => {}}
        onRemove={() => {}}
        isResetting={false}
        isRemoving={false}
      />,
    )

    const tokenBar = widthBars(container).find((b) => b.style.width === '100%')!
    // 100% clamps the width; the color bucket treats >= 90 as bg-error.
    expect(tokenBar.className).toContain('bg-error')
  })

  it('exposes all three consumer-visible actions and forwards clicks with the right payloads', async () => {
    const user = userEvent.setup()
    const onEdit = vi.fn()
    const onReset = vi.fn()
    const onRemove = vi.fn()
    const agent = makeAgent()

    render(
      <AgentBudgetCard
        agent={agent}
        onEdit={onEdit}
        onReset={onReset}
        onRemove={onRemove}
        isResetting={false}
        isRemoving={false}
      />,
    )

    const editBtn = screen.getByRole('button', { name: 'budget.editLimit' })
    const resetBtn = screen.getByRole('button', { name: 'budget.resetWindow' })
    const removeBtn = screen.getByRole('button', { name: 'budget.removeBudget' })

    expect(editBtn).toBeInTheDocument()
    expect(resetBtn).toBeInTheDocument()
    expect(removeBtn).toBeInTheDocument()

    await user.click(editBtn)
    expect(onEdit).toHaveBeenCalledTimes(1)
    expect(onEdit).toHaveBeenCalledWith(agent)

    await user.click(resetBtn)
    expect(onReset).toHaveBeenCalledTimes(1)
    expect(onReset).toHaveBeenCalledWith(agent.agent_id)

    await user.click(removeBtn)
    expect(onRemove).toHaveBeenCalledTimes(1)
    expect(onRemove).toHaveBeenCalledWith(agent.agent_id)
  })

  it('disables the reset and remove buttons while their mutations are pending', () => {
    render(
      <AgentBudgetCard
        agent={makeAgent()}
        onEdit={() => {}}
        onReset={() => {}}
        onRemove={() => {}}
        isResetting={true}
        isRemoving={true}
      />,
    )

    expect(screen.getByRole('button', { name: 'budget.editLimit' })).not.toBeDisabled()
    expect(screen.getByRole('button', { name: 'budget.resetWindow' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'budget.removeBudget' })).toBeDisabled()
  })

  it('renders the "no budget set" branch with only a set-budget action when has_budget=false', async () => {
    const user = userEvent.setup()
    const onEdit = vi.fn()
    const onReset = vi.fn()
    const onRemove = vi.fn()

    render(
      <AgentBudgetCard
        agent={makeAgent({ has_budget: false })}
        onEdit={onEdit}
        onReset={onReset}
        onRemove={onRemove}
        isResetting={false}
        isRemoving={false}
      />,
    )

    expect(screen.getByText('budget.noBudgetSet')).toBeInTheDocument()
    const setBtn = screen.getByRole('button', { name: 'budget.setBudget' })
    expect(setBtn).toBeInTheDocument()
    expect(screen.queryByRole('button', { name: 'budget.editLimit' })).toBeNull()
    expect(screen.queryByRole('button', { name: 'budget.resetWindow' })).toBeNull()
    expect(screen.queryByRole('button', { name: 'budget.removeBudget' })).toBeNull()

    await user.click(setBtn)
    expect(onEdit).toHaveBeenCalledTimes(1)
    expect(onReset).not.toHaveBeenCalled()
    expect(onRemove).not.toHaveBeenCalled()
  })
})
